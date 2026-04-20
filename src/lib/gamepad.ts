// Minimal gamepad reader that emits directional "pulses" and button presses.
// Uses repeat-on-hold with an initial delay, like a keyboard.

type Dir = "up" | "down" | "left" | "right";

export interface GamepadHandlers {
  onDir: (dir: Dir) => void;
  onConfirm: () => void;
  onCancel?: () => void;
  onSecondary?: () => void;
}

const DEADZONE = 0.4;
const INITIAL_REPEAT_MS = 380;
const REPEAT_MS = 110;

export function startGamepadLoop(h: GamepadHandlers): () => void {
  let raf = 0;
  const next: Record<Dir, number> = { up: 0, down: 0, left: 0, right: 0 };
  const held: Record<Dir, boolean> = { up: false, down: false, left: false, right: false };
  const buttonHeld: Record<number, boolean> = {};

  const fireDir = (dir: Dir, now: number) => {
    if (!held[dir]) {
      held[dir] = true;
      h.onDir(dir);
      next[dir] = now + INITIAL_REPEAT_MS;
    } else if (now >= next[dir]) {
      h.onDir(dir);
      next[dir] = now + REPEAT_MS;
    }
  };

  const releaseDir = (dir: Dir) => {
    held[dir] = false;
    next[dir] = 0;
  };

  const firePressOnce = (pad: Gamepad, idx: number, action?: () => void) => {
    const btn = pad.buttons[idx];
    if (!btn || !action) return;
    const pressed = btn.pressed || btn.value > 0.5;
    const key = idx;
    if (pressed && !buttonHeld[key]) {
      buttonHeld[key] = true;
      action();
    } else if (!pressed && buttonHeld[key]) {
      buttonHeld[key] = false;
    }
  };

  const tick = () => {
    const now = performance.now();
    const pads = navigator.getGamepads ? navigator.getGamepads() : [];
    let x = 0, y = 0;
    const dpad = { up: false, down: false, left: false, right: false };
    for (const pad of pads) {
      if (!pad) continue;
      const sx = pad.axes[0] ?? 0;
      const sy = pad.axes[1] ?? 0;
      if (Math.abs(sx) > Math.abs(x)) x = sx;
      if (Math.abs(sy) > Math.abs(y)) y = sy;
      dpad.up ||= !!pad.buttons[12]?.pressed;
      dpad.down ||= !!pad.buttons[13]?.pressed;
      dpad.left ||= !!pad.buttons[14]?.pressed;
      dpad.right ||= !!pad.buttons[15]?.pressed;
      // Standard mapping: 0=A, 1=B, 2=X, 3=Y
      firePressOnce(pad, 0, h.onConfirm);
      firePressOnce(pad, 1, h.onCancel);
      firePressOnce(pad, 3, h.onSecondary);
    }

    const up = dpad.up || y < -DEADZONE;
    const down = dpad.down || y > DEADZONE;
    const left = dpad.left || x < -DEADZONE;
    const right = dpad.right || x > DEADZONE;

    if (up) fireDir("up", now); else releaseDir("up");
    if (down) fireDir("down", now); else releaseDir("down");
    if (left) fireDir("left", now); else releaseDir("left");
    if (right) fireDir("right", now); else releaseDir("right");

    raf = requestAnimationFrame(tick);
  };

  raf = requestAnimationFrame(tick);
  return () => cancelAnimationFrame(raf);
}
