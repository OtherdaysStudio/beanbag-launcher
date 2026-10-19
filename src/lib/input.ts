import type { Action, Direction } from "./types";

/** Standard-mapping button index → action. Xbox: A B X Y LB RB View Menu. */
const BUTTONS: ReadonlyArray<[number, Action]> = [
  [0, "a"],
  [1, "b"],
  [2, "x"],
  [3, "y"],
  [4, "lb"],
  [5, "rb"],
  [8, "menu"],
  [9, "menu"],
];
const DPAD: ReadonlyArray<[number, Direction]> = [
  [12, "up"],
  [13, "down"],
  [14, "left"],
  [15, "right"],
];

const STICK_ON = 0.55;
const STICK_OFF = 0.35;
const REPEAT_DELAY = 320;
const REPEAT_SLOW = 110;
const REPEAT_FAST = 55;
const REPEAT_RAMP = 900;

const KEYS: Record<string, Action> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  Enter: "a",
  " ": "a",
  Escape: "b",
  Backspace: "b",
  "/": "y",
};

export type Device = "pad" | "keys";

/** One notch of a mouse wheel is about 100px; a trackpad sends many small deltas. */
const WHEEL_STEP = 90;

/** Direction from one pad, with hysteresis on the stick so it does not flicker at the edge. */
function padDirection(pad: Gamepad, held: Direction | null): Direction | null {
  for (const [i, dir] of DPAD) if (pad.buttons[i]?.pressed) return dir;
  const x = pad.axes[0] ?? 0;
  const y = pad.axes[1] ?? 0;
  const mag = Math.max(Math.abs(x), Math.abs(y));
  if (mag < (held ? STICK_OFF : STICK_ON)) return null;
  if (Math.abs(x) > Math.abs(y)) return x > 0 ? "right" : "left";
  return y > 0 ? "down" : "up";
}

/**
 * Reads the controller every frame, the keyboard on keydown and the mouse wheel.
 * `onKey` sees every key first; return true to consume it (text entry, shortcuts).
 * `onDevice` hears which device was used last, so hints can match it.
 */
export function startInput(
  onAction: (action: Action) => void,
  onKey?: (e: KeyboardEvent) => boolean,
  onDevice?: (device: Device) => void,
): () => void {
  let frame = 0;
  let pressed = new Set<number>();
  let armed = false;
  let held: Direction | null = null;
  let heldSince = 0;
  let nextRepeat = 0;

  const tick = (now: number) => {
    frame = requestAnimationFrame(tick);
    const pads = navigator.getGamepads?.() ?? [];
    const focused = document.hasFocus();

    const down = new Set<number>();
    let dir: Direction | null = null;
    for (const pad of pads) {
      if (!pad || !pad.connected) continue;
      for (const [i] of BUTTONS) if (pad.buttons[i]?.pressed) down.add(i);
      dir ??= padDirection(pad, held);
    }

    // After focus returns (a game closed), ignore whatever is still held.
    if (!focused || !armed) {
      pressed = down;
      held = dir;
      heldSince = now;
      nextRepeat = now + REPEAT_DELAY;
      armed = focused;
      return;
    }

    for (const [i, action] of BUTTONS) {
      if (down.has(i) && !pressed.has(i)) {
        onDevice?.("pad");
        onAction(action);
      }
    }
    pressed = down;

    if (dir !== held) {
      held = dir;
      heldSince = now;
      nextRepeat = now + REPEAT_DELAY;
      if (dir) {
        onDevice?.("pad");
        onAction(dir);
      }
    } else if (dir && now >= nextRepeat) {
      const t = Math.min(1, (now - heldSince - REPEAT_DELAY) / REPEAT_RAMP);
      nextRepeat = now + REPEAT_SLOW + (REPEAT_FAST - REPEAT_SLOW) * t;
      onAction(dir);
    }
  };

  const keydown = (e: KeyboardEvent) => {
    onDevice?.("keys");
    if (onKey?.(e)) {
      e.preventDefault();
      return;
    }
    if (e.ctrlKey && e.key.toLowerCase() === "f") {
      e.preventDefault();
      onAction("y");
      return;
    }
    const action = KEYS[e.key];
    if (!action || e.ctrlKey || e.metaKey || e.altKey) return;
    e.preventDefault();
    onAction(action);
  };

  let wheel = 0;
  const onWheel = (e: WheelEvent) => {
    if (e.ctrlKey) return;
    wheel += e.deltaY;
    while (Math.abs(wheel) >= WHEEL_STEP) {
      onDevice?.("keys");
      onAction(wheel > 0 ? "down" : "up");
      wheel -= Math.sign(wheel) * WHEEL_STEP;
    }
  };

  frame = requestAnimationFrame(tick);
  window.addEventListener("keydown", keydown);
  window.addEventListener("wheel", onWheel, { passive: true });
  return () => {
    cancelAnimationFrame(frame);
    window.removeEventListener("keydown", keydown);
    window.removeEventListener("wheel", onWheel);
  };
}
