export type Point = {
  x: number,
  y: number,
};

export type Action = {
  Click: {
    name: string,
    content: string,
    point: Point,
  }
} | {
  TypeText: {
    format: "Text" | "Email" | "Number"
  }
}
  | "PressKey"
  | {
    ScrollUp: {
      origin: {
        x: number,
        y: number,
      },
      distance: number,
    }
  };

export type Weight = number;

export type Timeout = number;

export type Actions = [Weight, Timeout, Action][];
