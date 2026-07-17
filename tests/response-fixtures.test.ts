import { describe, expect, it } from "vitest";
import { AgentResponseSchema } from "@/types/agent";

const messageFixture = {
  schemaVersion: "1",
  assistantMessage: "Here is the explanation...",
  responseType: "message",
  toolChange: null,
};

const toolChangeFixture = {
  schemaVersion: "1",
  assistantMessage: "I created a simple water tracker for you.",
  responseType: "tool_change",
  toolChange: {
    action: "create",
    targetToolId: null,
    tool: {
      id: "water-tracker",
      name: "Water Tracker",
      description: "Tracks daily glasses of water.",
      layout: { type: "single-column" },
      components: [
        {
          id: "card-main",
          type: "card",
          children: [
            {
              id: "title",
              type: "heading",
              props: { text: "Water Tracker", level: 2 },
            },
            {
              id: "water-count",
              type: "counter",
              valueKey: "count",
              props: { label: "Glasses", min: 0 },
            },
            {
              id: "inc",
              type: "button",
              props: { label: "Add glass", variant: "primary" },
              actions: [{ type: "increment", target: "count", amount: 1 }],
            },
          ],
        },
      ],
    },
    changeSummary: "Create water tracker",
  },
  diagnostics: {
    promptVersion: "coreside-prompt-v1",
    provider: "gemini",
    model: "gemini-2.0-flash",
  },
};

const quizFixture = {
  schemaVersion: "1",
  assistantMessage: "Here is a short quiz.",
  responseType: "tool_change",
  toolChange: {
    action: "create",
    targetToolId: null,
    tool: {
      id: "sample-quiz",
      name: "Sample Quiz",
      description: "A short quiz",
      components: [
        {
          id: "quiz-root",
          type: "quiz",
          valueKey: "quiz",
          props: {
            questions: [
              {
                id: "q1",
                prompt: "What is 2 + 2?",
                choices: ["3", "4", "5"],
                correctIndex: 1,
                explanation: "Two plus two equals four.",
              },
            ],
          },
        },
      ],
    },
    changeSummary: "Create quiz",
  },
};

const noopFixture = {
  schemaVersion: "1",
  assistantMessage: "No changes needed.",
  responseType: "noop",
};

describe("agent response fixtures", () => {
  it("parses a normal message response", () => {
    const parsed = AgentResponseSchema.parse(messageFixture);
    expect(parsed.responseType).toBe("message");
    expect(parsed.toolChange).toBeNull();
  });

  it("parses a tool_change response", () => {
    const parsed = AgentResponseSchema.parse(toolChangeFixture);
    expect(parsed.responseType).toBe("tool_change");
    expect(parsed.toolChange?.tool.id).toBe("water-tracker");
    expect(parsed.toolChange?.tool.components[0]?.type).toBe("card");
    expect(parsed.diagnostics?.promptVersion).toBe("coreside-prompt-v1");
  });

  it("parses a quiz tool fixture", () => {
    const parsed = AgentResponseSchema.parse(quizFixture);
    expect(parsed.toolChange?.tool.components[0]?.type).toBe("quiz");
  });

  it("parses noop responses", () => {
    const parsed = AgentResponseSchema.parse(noopFixture);
    expect(parsed.responseType).toBe("noop");
  });

  it("rejects invalid response types", () => {
    expect(() =>
      AgentResponseSchema.parse({
        assistantMessage: "x",
        responseType: "stream",
      }),
    ).toThrow();
  });
});
