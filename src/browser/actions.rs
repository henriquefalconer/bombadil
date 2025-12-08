use std::time::Duration;

use anyhow::{anyhow, bail};
use chromiumoxide::cdp::browser_protocol::{input, page};
use chromiumoxide::Page;
use include_dir::{include_dir, Dir};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json as json;

use crate::browser::actions::keys::key_name;
use crate::browser::actions::tree::{Tree, Weight};
use crate::browser::state::BrowserState;
use crate::geometry::Point;

pub mod keys;
pub mod tree;

#[allow(unused, reason = "some fields are useful for debugging")]
#[derive(Clone, Debug, Deserialize)]
pub enum BrowserAction {
    Back,
    Click {
        name: String,
        content: Option<String>,
        point: Point,
    },
    TypeText {
        text: String,
    },
    PressKey {
        code: u8,
    },
    ScrollUp {
        origin: Point,
        distance: f64,
    },
    ScrollDown {
        origin: Point,
        distance: f64,
    },
    Reload,
}

type WithTimeout<T> = (T, Duration);

impl BrowserAction {
    pub async fn apply(&self, page: &Page) -> anyhow::Result<()> {
        match self {
            BrowserAction::Back => {
                let history =
                    page.execute(page::GetNavigationHistoryParams {}).await?;
                if history.current_index == 0 {
                    bail!("can't go back from first navigation entry");
                }
                let last: page::NavigationEntry = history.entries
                    [(history.current_index - 1) as usize]
                    .clone();
                page.execute(
                    page::NavigateToHistoryEntryParams::builder()
                        .entry_id(last.id)
                        .build()
                        .map_err(|err| anyhow!(err))?,
                )
                .await?;
            }
            BrowserAction::Reload => {
                page.reload().await?;
            }
            BrowserAction::ScrollUp { origin, distance } => {
                page.execute(
                    input::SynthesizeScrollGestureParams::builder()
                        .x(origin.x)
                        .y(origin.y)
                        .y_distance(*distance)
                        .speed((distance.abs() * 10.0) as i64)
                        .build()
                        .map_err(|err| anyhow!(err))?,
                )
                .await?;
            }
            BrowserAction::ScrollDown { origin, distance } => {
                page.execute(
                    input::SynthesizeScrollGestureParams::builder()
                        .x(origin.x)
                        .y(origin.y)
                        .y_distance(-distance)
                        .speed((distance.abs() * 10.0) as i64)
                        .build()
                        .map_err(|err| anyhow!(err))?,
                )
                .await?;
            }
            BrowserAction::Click { point, .. } => {
                page.click((*point).into()).await?;
            }
            BrowserAction::TypeText { text } => {
                // TODO: maybe dispatch key presses instead with some random timing inbetween
                page.execute(input::InsertTextParams::new(text)).await?;
            }
            BrowserAction::PressKey { code } => {
                let build_params = |event_type| {
                    if let Some(name) = key_name(*code) {
                        input::DispatchKeyEventParams::builder()
                            .r#type(event_type)
                            .native_virtual_key_code(*code as i64)
                            .windows_virtual_key_code(*code as i64)
                            .code(name)
                            .key(name)
                            .unmodified_text("\r")
                            .text("\r")
                            .build()
                            .map_err(|err| anyhow!(err))
                    } else {
                        bail!("unknown key with code: {:?}", code)
                    }
                };
                page.execute(build_params(
                    input::DispatchKeyEventType::RawKeyDown,
                )?)
                .await?;
                page.execute(build_params(input::DispatchKeyEventType::Char)?)
                    .await?;
                page.execute(build_params(input::DispatchKeyEventType::KeyUp)?)
                    .await?;
            }
        };
        Ok(())
    }
}

static ACTIONS_DIR: Dir =
    include_dir!("$CARGO_MANIFEST_DIR/src/browser/actions");

async fn run_script<Input: Into<json::Value>, Output: DeserializeOwned>(
    state: &BrowserState,
    name: impl Into<&str>,
    input: Input,
) -> anyhow::Result<Output> {
    let script_path = format!("{}.js", name.into());
    let script_file = ACTIONS_DIR
        .get_file(&script_path)
        .ok_or(anyhow!("missing script {}", script_path))?;

    let script_contents = script_file
        .contents_utf8()
        .ok_or(anyhow!("failed to get script contents"))?;

    state
        .evaluate_function_call(script_contents, vec![input.into()])
        .await
}

async fn run_actions_script(
    state: &BrowserState,
    name: impl Into<&str>,
) -> anyhow::Result<Vec<(Weight, Tree<WithTimeout<BrowserAction>>)>> {
    let actions: Vec<(Weight, u64, BrowserAction)> =
        run_script(state, name, ()).await?;
    Ok(actions
        .iter()
        .map(|(weight, timeout_ms, action)| {
            (
                *weight,
                Tree::Leaf((
                    action.clone(),
                    Duration::from_millis(*timeout_ms),
                )),
            )
        })
        .collect::<Vec<_>>())
}

pub async fn available_actions(
    state: &BrowserState,
) -> anyhow::Result<Tree<WithTimeout<BrowserAction>>> {
    let tree = Tree::Branch(vec![
        (2, Tree::Branch(run_actions_script(state, "clicks").await?)),
        (2, Tree::Branch(run_actions_script(state, "inputs").await?)),
        (1, Tree::Branch(run_actions_script(state, "scrolls").await?)),
    ])
    .prune();

    if state.content_type != "text/html" {
        return Ok(Tree::Leaf((BrowserAction::Back, Duration::from_secs(2))));
    }

    if let Some(tree) = tree {
        Ok(tree)
    } else {
        Ok(Tree::Branch(vec![
            (5, Tree::Leaf((BrowserAction::Back, Duration::from_secs(2)))),
            (
                1,
                Tree::Leaf((BrowserAction::Reload, Duration::from_secs(1))),
            ),
        ]))
    }
}
