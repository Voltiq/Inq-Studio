// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { Select } from "./Select";
import { setLanguage } from "../../lib/i18n";

const OPTIONS = [
  { value: "a4", label: "A4" },
  { value: "letter", label: "Letter" },
  { value: "legal", label: "Legal" },
];

function Harness({ onChange }: { onChange: (v: string) => void }) {
  const [value, setValue] = useState("a4");
  return (
    <Select
      label="Paper size"
      value={value}
      options={OPTIONS}
      onChange={(v) => {
        setValue(v);
        onChange(v);
      }}
    />
  );
}

beforeAll(() => {
  setLanguage("en");
  // jsdom does not lay anything out.
  Element.prototype.scrollIntoView = vi.fn();
});
afterEach(cleanup);

describe("Select", () => {
  it("shows the chosen option and opens a list of all of them", () => {
    render(<Harness onChange={() => {}} />);
    const trigger = screen.getByRole("combobox", { name: /Paper size/ });
    expect(trigger.textContent).toContain("A4");
    expect(screen.queryByRole("listbox")).toBeNull();
    fireEvent.click(trigger);
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["A4", "Letter", "Legal"]);
    expect(screen.getByRole("option", { name: "A4" }).getAttribute("aria-selected")).toBe("true");
  });

  it("chooses with the mouse and closes", () => {
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    fireEvent.click(screen.getByRole("combobox"));
    fireEvent.click(screen.getByRole("option", { name: "Legal" }));
    expect(onChange).toHaveBeenCalledWith("legal");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(screen.getByRole("combobox").textContent).toContain("Legal");
  });

  it("works from the keyboard: arrows, Enter, type-ahead and Escape", () => {
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    const trigger = screen.getByRole("combobox");
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    expect(screen.getByRole("listbox")).toBeTruthy();
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.keyDown(trigger, { key: "Enter" });
    expect(onChange).toHaveBeenLastCalledWith("letter");

    fireEvent.keyDown(trigger, { key: "Enter" });
    fireEvent.keyDown(trigger, { key: "l" });
    fireEvent.keyDown(trigger, { key: "e" });
    fireEvent.keyDown(trigger, { key: "g" });
    fireEvent.keyDown(trigger, { key: " " });
    expect(onChange).toHaveBeenLastCalledWith("legal");

    // Escape closes the list and stays inside it: a dialog around it must not close too.
    const outer = vi.fn();
    cleanup();
    render(
      <div onKeyDown={outer}>
        <Harness onChange={onChange} />
      </div>,
    );
    const again = screen.getByRole("combobox");
    fireEvent.click(again);
    fireEvent.keyDown(again, { key: "Escape" });
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(outer).not.toHaveBeenCalled();
  });

  it("closes when something else is pressed", () => {
    render(<Harness onChange={() => {}} />);
    fireEvent.click(screen.getByRole("combobox"));
    fireEvent.pointerDown(document.body);
    expect(screen.queryByRole("listbox")).toBeNull();
  });
});
