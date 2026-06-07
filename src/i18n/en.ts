const en = {
  title: "CCTV",

  // State labels
  state: {
    working: "Working",
    waiting_permission: "Waiting for permission",
    waiting_input: "Waiting for input",
    idle: "Done",
    unknown: "No signal",
    completed: "Completed",
    error: "Error",
  },

  // Summary bar
  summary: {
    instances_one: "{{count}} instance",
    instances_other: "{{count}} instances",
    attention_one: "{{count}} needs attention",
    attention_other: "{{count}} need attention",
  },

  // Click-to-copy feedback
  copied: "Copied",

  // Empty state
  empty: "No active instances",
} as const;

export default en;
