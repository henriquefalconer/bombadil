function type_text() {
  let element = document.activeElement;

  debugger;
  if (element === undefined || element === null || element === document.body) {
    return [];
  }

  if (element.tagName === "TEXTAREA") {
    return [
      [1, 50, { TypeText: { text: "testing" }}],
      [1, 50, { TypeText: { text: "\t" }}],
      [1, 50, { TypeText: { text: "\n" }}],
      [1, 50, { TypeText: { text: " " }}],
    ];
  }

  if (element.tagName === "INPUT") {
    switch (element.type) {
      case "text":
        return [
          [1, 50, { TypeText: { text: "testing" } }],
          [1, 50, { TypeText: { text: " " } }],
          [1, 50, { TypeText: { text: "\t" } }],
          [1, 50, { TypeText: { text: "❤️" } }],
          [1, 50, { PressKey: { code: 13 } }], // Enter
        ];
      case "number":
        return [[1, 50, { TypeText: { text: "0" }}]]; // TODO: better numbers (in range, etc)
      // TODO: support other types
      default:
        return [];
    }
  }

  return [];
}
