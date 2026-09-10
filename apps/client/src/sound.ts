const key = "honkoku.sound";
let enabled = true;
try {
  enabled = localStorage.getItem(key) !== "off";
} catch {}
export const soundEnabled = () => enabled;
export function setSound(value: boolean) {
  enabled = value;
  try {
    localStorage.setItem(key, value ? "on" : "off");
  } catch {}
}
let context: AudioContext | undefined;
export function prepareSound() {
  if (!enabled) return;
  try {
    context ??= new AudioContext();
    void context.resume().catch(() => {});
  } catch {}
}
export function completionSound() {
  if (!enabled || !context || context.state !== "running") return;
  const start = context.currentTime;
  for (const [i, frequency] of [660, 880].entries()) {
    const tone = context.createOscillator();
    const gain = context.createGain();
    const at = start + i * 0.14;
    tone.type = "sine";
    tone.frequency.value = frequency;
    gain.gain.setValueAtTime(0, at);
    gain.gain.linearRampToValueAtTime(0.07, at + 0.015);
    gain.gain.linearRampToValueAtTime(0, at + 0.12);
    tone.connect(gain).connect(context.destination);
    tone.start(at);
    tone.stop(at + 0.12);
    tone.onended = () => {
      tone.disconnect();
      gain.disconnect();
    };
  }
}
