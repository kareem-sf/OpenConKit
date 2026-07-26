import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Icon } from "./Icon";

describe("Icon", () => {
  it("renders icons at 75% of their requested footprint", () => {
    const { container } = render(<Icon name="home" size={20} />);
    const icon = container.querySelector("svg");

    expect(icon?.getAttribute("width")).toBe("15");
    expect(icon?.getAttribute("height")).toBe("15");
  });
});
