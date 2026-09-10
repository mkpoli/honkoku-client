import type { createEditor } from "@honkoku/editor";
export {
  fromMarkup,
  historyKey,
  toMarkup,
  textareaSource,
  TextSelection,
  type EditorUpdate,
} from "@honkoku/editor";
declare global {
  interface Window {
    editorSpike?: ReturnType<typeof createEditor> & {
      readonly source: string;
      fixture: { text: string; title: string; index: number };
    };
  }
}
