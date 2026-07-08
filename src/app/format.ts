/** Format an idle duration in seconds for the status display. */
export function formatIdle(secs: number): string {
  if (secs < 60) return `Idle: ${Math.floor(secs)}s`;
  if (secs < 3600)
    return `Idle: ${Math.floor(secs / 60)}m ${Math.floor(secs % 60)}s`;
  return `Idle: ${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
}
