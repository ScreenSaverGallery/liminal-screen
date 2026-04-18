/**
 * Minimal reactive signal — similar to Solid.js createSignal or a Svelte writable store.
 *
 * Usage:
 *   const count = new Signal(0);
 *   count.effect(v => console.log('count is', v)); // runs immediately + on every change
 *   count.set(1);  // logs "count is 1"
 *   count.update(n => n + 1); // logs "count is 2"
 */
export class Signal<T> {
  private _value: T;
  private effects: Set<(value: T) => void> = new Set();

  constructor(initial: T) {
    this._value = initial;
  }

  get(): T {
    return this._value;
  }

  set(value: T): void {
    this._value = value;
    this.effects.forEach((fn) => fn(value));
  }

  update(fn: (current: T) => T): void {
    this.set(fn(this._value));
  }

  /**
   * Register an effect. Runs immediately with the current value,
   * then re-runs on every future set/update.
   * Returns a cleanup function that removes the effect.
   */
  effect(fn: (value: T) => void): () => void {
    this.effects.add(fn);
    fn(this._value);
    return () => this.effects.delete(fn);
  }
}
