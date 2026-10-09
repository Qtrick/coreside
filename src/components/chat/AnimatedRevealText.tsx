import { useEffect, useRef, useState } from "react";

/** Steady reveal rate so network chunks still look letter-by-letter. */
const CHARS_PER_SEC = 52;
/** Stagger within the live tail so glyphs cascade instead of popping together. */
const CHAR_STAGGER_MS = 16;

/**
 * Reveals `text` at a controlled pace with per-glyph fade/blur-in.
 * Source may arrive in large SSE chunks; display stays smooth.
 *
 * ponytail: only the live tail is span-wrapped; committed prefix is one text node.
 */
export function AnimatedRevealText({
  text,
  className,
  onCaughtUp,
}: {
  text: string;
  className?: string;
  /** Fires when displayed length has caught the current target (and stays caught). */
  onCaughtUp?: () => void;
}) {
  const [displayed, setDisplayed] = useState("");
  const [freshFrom, setFreshFrom] = useState(0);
  const displayedRef = useRef("");
  const targetRef = useRef(text);
  const onCaughtUpRef = useRef(onCaughtUp);
  const wasBehindRef = useRef(false);
  targetRef.current = text;
  onCaughtUpRef.current = onCaughtUp;

  useEffect(() => {
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduced) {
      displayedRef.current = text;
      setDisplayed(text);
      setFreshFrom(text.length);
      onCaughtUpRef.current?.();
      return;
    }

    let raf = 0;
    let last = performance.now();
    let carry = 0;
    let alive = true;

    const step = (now: number) => {
      if (!alive) return;
      const target = targetRef.current;
      let cur = displayedRef.current;

      if (!target.startsWith(cur)) {
        cur = "";
        displayedRef.current = "";
        setDisplayed("");
        setFreshFrom(0);
        carry = 0;
        last = now;
        wasBehindRef.current = target.length > 0;
      }

      if (cur.length < target.length) {
        wasBehindRef.current = true;
        carry += ((now - last) / 1000) * CHARS_PER_SEC;
        last = now;
        const n = Math.floor(carry);
        if (n >= 1) {
          carry -= n;
          const nextLen = Math.min(target.length, cur.length + n);
          setFreshFrom(cur.length);
          const next = target.slice(0, nextLen);
          displayedRef.current = next;
          setDisplayed(next);
          if (nextLen >= target.length && wasBehindRef.current) {
            wasBehindRef.current = false;
            onCaughtUpRef.current?.();
          }
        }
      } else {
        last = now;
        carry = 0;
        if (wasBehindRef.current && cur.length >= target.length) {
          wasBehindRef.current = false;
          onCaughtUpRef.current?.();
        }
      }

      raf = requestAnimationFrame(step);
    };

    raf = requestAnimationFrame(step);
    return () => {
      alive = false;
      cancelAnimationFrame(raf);
    };
  }, []);

  // Kick catch-up when a reduced-motion user gets new text after mount.
  useEffect(() => {
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (!reduced) return;
    displayedRef.current = text;
    setDisplayed(text);
    setFreshFrom(text.length);
    onCaughtUpRef.current?.();
  }, [text]);

  const solid = displayed.slice(0, freshFrom);
  const animating = displayed.slice(freshFrom);

  return (
    <span className={className ? `animated-reveal-text ${className}` : "animated-reveal-text"}>
      {solid}
      {Array.from(animating).map((ch, i) => (
        <span
          key={freshFrom + i}
          className="stream-char"
          style={{ animationDelay: `${i * CHAR_STAGGER_MS}ms` }}
        >
          {ch}
        </span>
      ))}
    </span>
  );
}
