"use client";

import { motion, useReducedMotion } from "motion/react";

const EASE = [0.32, 0.72, 0, 1] as const;

/** Heavy fade-up on viewport entry; transform/opacity only. */
export function Reveal({
  children,
  delay = 0,
  className,
}: {
  children: React.ReactNode;
  delay?: number;
  className?: string;
}) {
  const reduce = useReducedMotion();
  return (
    <motion.div
      className={className}
      initial={reduce ? false : { opacity: 0, y: 28 }}
      transition={{ duration: 0.9, delay, ease: EASE }}
      viewport={{ once: true, margin: "-40px" }}
      whileInView={{ opacity: 1, y: 0 }}
    >
      {children}
    </motion.div>
  );
}
