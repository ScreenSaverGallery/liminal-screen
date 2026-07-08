import { describe, it, expect } from "vitest";
import { formatIdle } from "./format";

describe("formatIdle", () => {
  it("formats seconds under a minute", () => {
    expect(formatIdle(0)).toBe("Idle: 0s");
    expect(formatIdle(45)).toBe("Idle: 45s");
    expect(formatIdle(59.9)).toBe("Idle: 59s");
  });

  it("formats minutes and seconds under an hour", () => {
    expect(formatIdle(60)).toBe("Idle: 1m 0s");
    expect(formatIdle(150)).toBe("Idle: 2m 30s");
    expect(formatIdle(3599)).toBe("Idle: 59m 59s");
  });

  it("formats hours and minutes from an hour up", () => {
    expect(formatIdle(3600)).toBe("Idle: 1h 0m");
    expect(formatIdle(3900)).toBe("Idle: 1h 5m");
    expect(formatIdle(7325)).toBe("Idle: 2h 2m");
  });
});
