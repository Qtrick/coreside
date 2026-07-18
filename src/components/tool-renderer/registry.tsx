import type { ReactNode } from "react";
import type { ComponentType } from "@/types/tool";
import type { ToolNodeProps } from "./nodes";
import {
  BadgeNode,
  ButtonGroupNode,
  ButtonNode,
  CardNode,
  CheckboxNode,
  ChecklistNode,
  ClockNode,
  ColumnNode,
  ContainerNode,
  CounterNode,
  DateInputNode,
  DividerNode,
  EmptyStateNode,
  HeadingNode,
  ImageNode,
  ListNode,
  NumberInputNode,
  ProgressNode,
  QuizNode,
  RowNode,
  SelectNode,
  SpacerNode,
  StatNode,
  TableNode,
  TabsNode,
  TextAreaNode,
  TextInputNode,
  TextNode,
  UnsupportedNode,
} from "./nodes";

export const componentRegistry: Record<
  ComponentType,
  (props: ToolNodeProps) => ReactNode
> = {
  container: ContainerNode,
  row: RowNode,
  column: ColumnNode,
  card: CardNode,
  tabs: TabsNode,
  divider: DividerNode,
  spacer: SpacerNode,
  heading: HeadingNode,
  text: TextNode,
  badge: BadgeNode,
  image: ImageNode,
  emptyState: EmptyStateNode,
  textInput: TextInputNode,
  textArea: TextAreaNode,
  numberInput: NumberInputNode,
  select: SelectNode,
  checkbox: CheckboxNode,
  dateInput: DateInputNode,
  list: ListNode,
  checklist: ChecklistNode,
  table: TableNode,
  counter: CounterNode,
  progress: ProgressNode,
  stat: StatNode,
  button: ButtonNode,
  buttonGroup: ButtonGroupNode,
  quiz: QuizNode,
  clock: ClockNode,
};

export function resolveComponent(
  type: string,
): (props: ToolNodeProps) => ReactNode {
  if (type in componentRegistry) {
    return componentRegistry[type as ComponentType];
  }
  return UnsupportedNode;
}
