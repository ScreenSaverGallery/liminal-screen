import { describe, it, expect, vi } from "vitest";
import { Signal } from "./reactive";

describe("Signal", () => {
  it("holds initial value", () => {
    const s = new Signal(42);
    expect(s.get()).toBe(42);
  });

  it("set updates value", () => {
    const s = new Signal(0);
    s.set(5);
    expect(s.get()).toBe(5);
  });

  it("update applies transform", () => {
    const s = new Signal(3);
    s.update((n) => n * 2);
    expect(s.get()).toBe(6);
  });

  it("effect fires immediately with current value", () => {
    const s = new Signal("hello");
    const fn = vi.fn();
    s.effect(fn);
    expect(fn).toHaveBeenCalledWith("hello");
  });

  it("effect fires on each set", () => {
    const s = new Signal(0);
    const values: number[] = [];
    s.effect((v) => values.push(v));
    s.set(1);
    s.set(2);
    expect(values).toEqual([0, 1, 2]);
  });

  it("effect cleanup removes listener", () => {
    const s = new Signal(0);
    const fn = vi.fn();
    const cleanup = s.effect(fn);
    fn.mockClear();
    cleanup();
    s.set(99);
    expect(fn).not.toHaveBeenCalled();
  });

  it("derive produces computed child", () => {
    const s = new Signal(2);
    const doubled = s.derive((n) => n * 2);
    expect(doubled.get()).toBe(4);
    s.set(5);
    expect(doubled.get()).toBe(10);
  });

  it("multiple effects all fire", () => {
    const s = new Signal(0);
    const a = vi.fn(),
      b = vi.fn();
    s.effect(a);
    s.effect(b);
    a.mockClear();
    b.mockClear();
    s.set(1);
    expect(a).toHaveBeenCalledWith(1);
    expect(b).toHaveBeenCalledWith(1);
  });

  it("derive chain updates correctly", () => {
    const s = new Signal(1);
    const x2 = s.derive((n) => n * 2);
    const x4 = x2.derive((n) => n * 2);
    s.set(3);
    expect(x4.get()).toBe(12);
  });

  it("effects receive object identity on update", () => {
    const s = new Signal({ count: 0 });
    let seen: { count: number } | null = null;
    s.effect((v) => (seen = v));
    s.update((v) => ({ ...v, count: v.count + 1 }));
    expect(seen).toEqual({ count: 1 });
  });
});
