use rand::Rng;

pub type Weight = u16;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Tree<T> {
    Leaf(T),
    Branch(Vec<(Weight, Tree<T>)>),
}

impl<T: Clone> Tree<T> {
    pub fn pick<R: Rng>(&self, rng: &mut R) -> Option<T> {
        match self {
            Tree::Leaf(x) => Some(x.clone()),
            Tree::Branch(branches) => {
                let mut weight_total = 0;
                for (weight, _) in branches {
                    weight_total += weight;
                }
                let target = rng.random_range(0..weight_total);
                let mut current = 0;
                for (weight, action) in branches {
                    current += *weight;
                    if target < current {
                        return action.pick(rng);
                    }
                }
                None
            }
        }
    }

    fn prune_to_size(&mut self) -> usize {
        match self {
            Tree::Leaf(_) => 1,
            Tree::Branch(trees) => {
                let mut i = 0;
                while i < trees.len() {
                    if trees[i].1.prune_to_size() == 0 {
                        trees.remove(i);
                    } else {
                        i += 1;
                    }
                }

                return trees.len();
            }
        }
    }

    pub fn prune(mut self) -> Option<Self> {
        if self.prune_to_size() == 0 {
            None
        } else {
            Some(self)
        }
    }
}

#[cfg(test)]
mod tests {

    use super::Tree::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_pick_any() {
        let tree = Branch(vec![
            (1, Leaf(1)),
            (1, Branch(vec![(2, Leaf(2)), (3, Leaf(3))])),
        ]);
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        if let Some(pick) = tree.pick(&mut rng) {
            assert!(pick > 0 && pick < 4);
        } else {
            panic!("no pick");
        }
    }

    #[test]
    fn test_prune_non_empty() {
        let actual = Branch(vec![
            (1, Leaf(1)),
            (
                2,
                Branch(vec![(2, Leaf(2)), (3, Leaf(3)), (4, Branch(vec![]))]),
            ),
            (1, Branch(vec![])),
        ])
        .prune()
        .unwrap();
        let expected = Branch(vec![
            (1, Leaf(1)),
            (2, Branch(vec![(2, Leaf(2)), (3, Leaf(3))])),
        ]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_prune_empty() {
        let actual = Branch::<()>(vec![]).prune();
        assert_eq!(actual, None);
    }
}
