import type { ToolDefinition } from "@/types/tool";

export const FIXTURE_TOOLS: Record<string, ToolDefinition> = {
  dashboard: {
    id: "tool-finance-dashboard",
    name: "Personal Finance Dashboard",
    description: "Monthly income, expenses, and asset breakdown",
    layout: {
      type: "dashboard",
      columns: 4,
      gap: "md",
      responsive_collapse: true,
    },
    components: [
      {
        id: "dash-header",
        type: "heading",
        layout_role: "header",
        props: { text: "Financial Overview — September 2026", level: 2 },
      },
      {
        id: "stat-income",
        type: "card",
        layout_role: "stat",
        col_span: 1,
        props: {
          title: "Total Revenue",
          content: "$12,450.00 (+8.2%)",
        },
      },
      {
        id: "stat-expenses",
        type: "card",
        layout_role: "stat",
        col_span: 1,
        props: {
          title: "Operating Expenses",
          content: "$4,120.50 (-2.1%)",
        },
      },
      {
        id: "stat-net",
        type: "card",
        layout_role: "stat",
        col_span: 1,
        props: {
          title: "Net Savings",
          content: "$8,329.50",
        },
      },
      {
        id: "stat-runway",
        type: "card",
        layout_role: "stat",
        col_span: 1,
        props: {
          title: "Cash Runway",
          content: "18.4 Months",
        },
      },
      {
        id: "main-chart",
        type: "chartBar",
        layoutRole: "main",
        colSpan: 3,
        props: {
          data: [
            { label: "Jan", value: 8500 },
            { label: "Feb", value: 9200 },
            { label: "Mar", value: 11000 },
            { label: "Apr", value: 10400 },
            { label: "May", value: 12450 },
          ],
        },
      },
      {
        id: "side-breakdown",
        type: "card",
        layout_role: "sidebar",
        col_span: 1,
        props: {
          title: "Top Categories",
          content: "Housing: 42%\nFood & Dining: 18%\nSoftware: 14%\nTravel: 12%",
        },
      },
      {
        id: "recent-transactions",
        type: "dataTable",
        layout_role: "detail",
        col_span: 4,
        props: {
          title: "Recent Transactions",
          columns: [
            { id: "date", label: "Date" },
            { id: "desc", label: "Description" },
            { id: "category", label: "Category" },
            { id: "amount", label: "Amount" },
          ],
          rows: [
            { date: "2026-09-14", desc: "Cloud Hosting", category: "Software", amount: "-$120.00" },
            { date: "2026-09-12", desc: "Client Retainer", category: "Income", amount: "+$4,500.00" },
            { date: "2026-09-10", desc: "Office Supplies", category: "Operations", amount: "-$64.30" },
          ],
        },
      },
    ],
  },

  form: {
    id: "tool-lab-form",
    name: "Lab Experiment Entry Form",
    description: "Record parameters, sample IDs, and observations",
    layout: {
      type: "form",
      columns: 2,
      gap: "md",
      responsive_collapse: true,
    },
    components: [
      {
        id: "form-header",
        type: "heading",
        layout_role: "header",
        props: { text: "New Experiment Run", level: 2 },
      },
      {
        id: "input-sample-id",
        type: "textInput",
        col_span: 1,
        props: { label: "Sample Identifier", placeholder: "SMP-2026-0914" },
      },
      {
        id: "input-temp",
        type: "numberInput",
        col_span: 1,
        props: { label: "Incubation Temperature (°C)", defaultValue: 37 },
      },
      {
        id: "input-ph",
        type: "slider",
        col_span: 1,
        props: { label: "Buffer pH Level", min: 4, max: 10, step: 0.1, defaultValue: 7.4 },
      },
      {
        id: "input-reagent",
        type: "select",
        col_span: 1,
        props: {
          label: "Active Reagent",
          options: ["Reagent Alpha-1", "Reagent Beta-4", "Control Saline"],
        },
      },
      {
        id: "input-notes",
        type: "textArea",
        col_span: 2,
        props: { label: "Observation Notes", placeholder: "Record optical density and precipitate..." },
      },
      {
        id: "input-verified",
        type: "checkbox",
        col_span: 1,
        props: { label: "Protocol Verified by Lead" },
      },
      {
        id: "btn-submit",
        type: "button",
        layout_role: "actions",
        col_span: 2,
        props: { label: "Submit Experiment Data", variant: "primary" },
      },
    ],
  },

  tracker: {
    id: "tool-habit-tracker",
    name: "Weekly Habit Tracker",
    description: "Track daily engineering & health habits",
    layout: {
      type: "dashboard",
      columns: 3,
      gap: "sm",
    },
    components: [
      {
        id: "trk-header",
        type: "heading",
        layout_role: "header",
        props: { text: "Habit Tracker — Week 38", level: 2 },
      },
      {
        id: "habit-code",
        type: "checkbox",
        layout_role: "main",
        props: { label: "Review Rust Security Tests" },
      },
      {
        id: "habit-read",
        type: "checkbox",
        layout_role: "main",
        props: { label: "Read Architecture Papers (30m)" },
      },
      {
        id: "habit-walk",
        type: "checkbox",
        layout_role: "main",
        props: { label: "Afternoon Walk (5,000 steps)" },
      },
      {
        id: "stat-streak",
        type: "card",
        layout_role: "stat",
        props: { title: "Current Streak", content: "14 Days" },
      },
      {
        id: "stat-completion",
        type: "card",
        layout_role: "stat",
        props: { title: "Completion Rate", content: "92%" },
      },
    ],
  },

  calculator: {
    id: "tool-unit-calc",
    name: "Scientific Unit Converter & Calculator",
    description: "Quick calculations with history",
    layout: {
      type: "stack",
      gap: "md",
      max_width: "md",
    },
    components: [
      {
        id: "calc-header",
        type: "heading",
        props: { text: "Molar Mass Calculator", level: 2 },
      },
      {
        id: "calc-input",
        type: "textInput",
        props: { label: "Chemical Formula", placeholder: "e.g. C6H12O6" },
      },
      {
        id: "calc-btn",
        type: "button",
        props: { label: "Calculate", variant: "primary" },
      },
      {
        id: "calc-result",
        type: "card",
        props: { title: "Molar Mass", content: "180.16 g/mol" },
      },
    ],
  },

  research: {
    id: "tool-research-organizer",
    name: "Research Organizer",
    description: "Search findings, references, citations, and syntheses",
    layout: {
      type: "split",
      gap: "md",
    },
    components: [
      {
        id: "res-header",
        type: "heading",
        layoutRole: "header",
        props: { text: "Research Organizer & Knowledge Synthesizer", level: 2 },
      },
      {
        id: "res-search",
        type: "textInput",
        valueKey: "searchQuery",
        layoutRole: "sidebar",
        props: { label: "Search Query", placeholder: "Search papers..." },
      },
      {
        id: "res-search-btn",
        type: "button",
        layoutRole: "sidebar",
        props: { label: "Query Papers", variant: "primary" },
        actions: [
          {
            type: "invokeRegisteredAction",
            actionName: "local_data.query",
            input: { model: "papers" },
            resultKey: "paperResults",
          },
        ],
      },
      {
        id: "res-table",
        type: "dataTable",
        valueKey: "paperResults",
        layoutRole: "main",
        props: {
          title: "Indexed Papers",
          selectionKey: "selectedPaperId",
          columns: [
            { id: "id", label: "Paper ID" },
            { id: "title", label: "Title" },
            { id: "year", label: "Year" },
          ],
          rows: [
            { id: "P-101", title: "Declarative UI Containment Patterns", year: "2026" },
            { id: "P-102", title: "Fail-Closed State Validation in WebViews", year: "2025" },
          ],
        },
      },
      {
        id: "res-notes",
        type: "textArea",
        valueKey: "paperNotes",
        layoutRole: "main",
        props: {
          label: "Research Synthesis Notes",
          placeholder: "Enter structured notes, critique, or findings...",
        },
      },
      {
        id: "res-submit-btn",
        type: "button",
        layoutRole: "main",
        props: { label: "Submit Synthesis to Agent", variant: "secondary" },
        actions: [
          {
            type: "submitToAgent",
            eventName: "submit_paper_synthesis",
            includeFields: ["selectedPaperId", "paperNotes"],
          },
        ],
      },
    ],
  },

  quiz: {
    id: "tool-study-quiz",
    name: "Biology Study Quiz",
    description: "Interactive practice questions with feedback",
    layout: {
      type: "content",
      gap: "md",
      max_width: "lg",
    },
    components: [
      {
        id: "quiz-title",
        type: "heading",
        props: { text: "Cellular Respiration Quiz (Question 3 of 10)", level: 2 },
      },
      {
        id: "quiz-question",
        type: "text",
        props: { text: "Which stage of cellular respiration produces the greatest amount of ATP?" },
      },
      {
        id: "quiz-opt-1",
        type: "button",
        props: { label: "A. Glycolysis", variant: "secondary" },
      },
      {
        id: "quiz-opt-2",
        type: "button",
        props: { label: "B. Citric Acid Cycle", variant: "secondary" },
      },
      {
        id: "quiz-opt-3",
        type: "button",
        props: { label: "C. Oxidative Phosphorylation", variant: "primary" },
      },
      {
        id: "quiz-opt-4",
        type: "button",
        props: { label: "D. Fermentation", variant: "secondary" },
      },
    ],
  },

  chart_data: {
    id: "tool-chart-explorer",
    name: "Telemetry Data Explorer",
    description: "Time-series measurements and summary tables",
    layout: {
      type: "dashboard",
      columns: 2,
      gap: "md",
    },
    components: [
      {
        id: "chart-1",
        type: "chartLine",
        colSpan: 2,
        props: {
          data: [
            { label: "10:00", value: 45 },
            { label: "10:15", value: 52 },
            { label: "10:30", value: 48 },
            { label: "10:45", value: 78 },
            { label: "11:00", value: 65 },
          ],
        },
      },
      {
        id: "table-data",
        type: "dataTable",
        col_span: 2,
        props: {
          title: "System Metric Log",
          columns: [
            { id: "timestamp", label: "Timestamp" },
            { id: "cpu", label: "CPU %" },
            { id: "mem", label: "Memory (MB)" },
          ],
          rows: [
            { timestamp: "10:00:00", cpu: "12%", mem: "245" },
            { timestamp: "10:15:00", cpu: "18%", mem: "260" },
            { timestamp: "10:30:00", cpu: "15%", mem: "258" },
          ],
        },
      },
    ],
  },

  media_browser: {
    id: "tool-media-browser",
    name: "Media Browser",
    description: "Gallery cards with layout spans",
    layout: {
      type: "grid",
      columns: 3,
      gap: "sm",
    },
    components: [
      {
        id: "media-card-1",
        type: "card",
        col_span: 1,
        props: { title: "Aurora Waveform", content: "High-resolution atmospheric texture" },
      },
      {
        id: "media-card-2",
        type: "card",
        col_span: 1,
        props: { title: "Cyber Grid", content: "Matrix glyph rain animation loop" },
      },
      {
        id: "media-card-3",
        type: "card",
        col_span: 1,
        props: { title: "Deep Nebula", content: "Ambient particle drift simulation" },
      },
    ],
  },

  code_workbench: {
    id: "tool-code-workbench",
    name: "Snippet Workbench",
    description: "Code editing with live output panel",
    layout: {
      type: "full",
      gap: "md",
    },
    components: [
      {
        id: "code-header",
        type: "heading",
        layout_role: "header",
        props: { text: "TypeScript Validator Snippet", level: 3 },
      },
      {
        id: "code-area",
        type: "codeEditor",
        layout_role: "main",
        props: {
          language: "typescript",
          code: "export function isSafeInput(key: string): boolean {\n  return !key.includes('__proto__');\n}",
        },
      },
      {
        id: "code-output",
        type: "card",
        layout_role: "detail",
        props: { title: "Compiler Diagnostics", content: "0 errors, 0 warnings. Code is valid." },
      },
    ],
  },

  settings_config: {
    id: "tool-settings-config",
    name: "Workspace Configuration",
    description: "Dense controls for runtime preferences",
    layout: {
      type: "form",
      columns: 2,
      gap: "sm",
    },
    components: [
      {
        id: "cfg-hdr",
        type: "heading",
        layout_role: "header",
        props: { text: "Runtime Policy Configuration", level: 2 },
      },
      {
        id: "cfg-fail-closed",
        type: "switch",
        col_span: 1,
        props: { label: "Fail-Closed Input Filtering", checked: true },
      },
      {
        id: "cfg-motion",
        type: "switch",
        col_span: 1,
        props: { label: "Respect Reduced Motion", checked: true },
      },
      {
        id: "cfg-throttle",
        type: "slider",
        col_span: 2,
        props: { label: "Persistence Throttle (ms)", min: 100, max: 2000, step: 50, defaultValue: 300 },
      },
    ],
  },

  crud_task_manager: {
    id: "tool-task-manager",
    name: "Project Task Manager",
    description: "Full-featured CRUD data management with persistent actions and reactive metrics",
    layout: {
      type: "dashboard",
      columns: 3,
      gap: "md",
    },
    components: [
      {
        id: "crud-header",
        type: "heading",
        layoutRole: "header",
        props: { text: "Sprint Execution & Task Board", level: 2 },
      },
      {
        id: "stat-open-tasks",
        type: "card",
        layoutRole: "stat",
        colSpan: 1,
        props: {
          title: "Active Tasks",
          content: "12 Remaining",
        },
        actions: [
          {
            type: "decrement",
            target: "openTasks",
            amount: 1,
          },
        ],
      },
      {
        id: "stat-completed-tasks",
        type: "card",
        layoutRole: "stat",
        colSpan: 1,
        props: {
          title: "Completed",
          content: "28 Closed",
        },
        actions: [
          {
            type: "increment",
            target: "completedTasks",
            amount: 1,
          },
        ],
      },
      {
        id: "stat-velocity",
        type: "card",
        layoutRole: "stat",
        colSpan: 1,
        props: {
          title: "Velocity",
          content: "4.8 pts/day",
        },
      },
      {
        id: "input-task-title",
        type: "textInput",
        valueKey: "newTaskTitle",
        layoutRole: "sidebar",
        props: { label: "New Task Title", placeholder: "e.g. Audit SQLite schema" },
      },
      {
        id: "input-task-priority",
        type: "select",
        valueKey: "newTaskPriority",
        layoutRole: "sidebar",
        props: {
          label: "Priority",
          options: ["High", "Medium", "Low"],
        },
      },
      {
        id: "btn-add-task",
        type: "button",
        layoutRole: "sidebar",
        props: { label: "Create Task", variant: "primary" },
        actions: [
          {
            type: "invokeRegisteredAction",
            actionName: "local_data.write",
            input: { model: "tasks" },
            inputFromState: {
              title: "newTaskTitle",
              priority: "newTaskPriority",
            },
            resultKey: "lastTask",
          },
          {
            type: "invokeRegisteredAction",
            actionName: "local_data.query",
            input: { model: "tasks" },
            resultKey: "tasks",
          },
        ],
      },
      {
        id: "btn-delete-task",
        type: "button",
        layoutRole: "sidebar",
        props: { label: "Delete Selected", variant: "secondary" },
        actions: [
          {
            type: "invokeRegisteredAction",
            actionName: "local_data.delete",
            input: { model: "tasks" },
            inputFromState: {
              id: "selectedTaskId",
            },
          },
          {
            type: "invokeRegisteredAction",
            actionName: "local_data.query",
            input: { model: "tasks" },
            resultKey: "tasks",
          },
        ],
      },
      {
        id: "tasks-data-table",
        type: "dataTable",
        valueKey: "tasks",
        layoutRole: "main",
        colSpan: 2,
        props: {
          title: "Sprint Tasks",
          selectionKey: "selectedTaskId",
          columns: [
            { id: "id", label: "Task ID" },
            { id: "title", label: "Title" },
            { id: "priority", label: "Priority" },
            { id: "status", label: "Status" },
          ],
          rows: [
            { id: "TSK-101", title: "Action authority enforcement", priority: "High", status: "Done" },
            { id: "TSK-102", title: "Layout engine container queries", priority: "Medium", status: "In Progress" },
            { id: "TSK-103", title: "Wallpaper visibility lifecycle", priority: "High", status: "Done" },
          ],
        },
      },
    ],
  },
};
