export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };
export type Timestamp = string;
export type PageStatus = "default" | "initiated" | "editing" | "completed" | "frozen" | (string & {});
export type Label = string | Record<string, string[]>;
export type Keywords = string | string[];

export interface Project {
  [key: string]: unknown;
  id: string;
  title: string;
  description?: string | null;
  keywords?: Keywords | null;
  markdown?: string | null;
  photo?: string | null;
  ownerId?: string | null;
  admins?: Array<string> | null;
  members?: Array<string> | null;
  blockedUsers?: Array<string> | null;
  projectType?: string | null;
  display?: boolean | null;
  useOwnGuidelines?: boolean | null;
  guidelines?: string | null;
  functions?: ProjectFunctions | null;
  totalEntryCount?: number | null;
  completedEntryCount?: number | null;
  totalImageCount?: number | null;
  completedImageCount?: number | null;
  charCount?: number | null;
  createdAt?: Timestamp | null;
  updatedAt?: Timestamp | null;
  collections?: Array<string> | null;
}

export interface ProjectFunctions {
  [key: string]: unknown;
  enableOCR?: boolean | null;
  enableTranslations?: boolean | null;
}

export interface Collection {
  [key: string]: unknown;
  id: string;
  projectId: string;
  title: string;
  description?: string | null;
  entryCount?: number | null;
  display?: boolean | null;
  entries?: Array<string> | null;
  updatedAt?: Timestamp | null;
}

export interface Entry {
  [key: string]: unknown;
  id: string;
  projectId: string;
  collectionId: string;
  index: number;
  label: Label;
  manifestUrl: string;
  thumbnail?: string | null;
  size?: number | null;
  createdAt?: Timestamp | null;
  updatedAt?: Timestamp | null;
  canvases?: Array<Canvas> | null;
  transcriptions?: Array<Page> | null;
}

export interface Canvas {
  [key: string]: unknown;
  id: string;
  width: number;
  height: number;
  infoJsonUrl?: string | null;
  imageUrl?: string | null;
  thumbnailUrl?: string | null;
}

export interface Page {
  [key: string]: unknown;
  id: string;
  entryId: string;
  index: number;
  canvasId?: string | null;
  status: PageStatus;
  prevStatus?: PageStatus | null;
  text: string;
  tempText?: string | null;
  tempTextChanged?: boolean | null;
  notes: Array<JsonValue | null>;
  tempNotes?: Array<JsonValue | null> | null;
  annotations?: Array<JsonValue> | null;
  translations?: JsonValue | null;
  ocr?: Ocr | null;
  editedBy?: string | null;
  approvedBy?: Array<string> | null;
  share?: boolean | null;
  requestReview?: boolean | null;
  syncMode?: boolean | null;
  tempEditedBy?: string | null;
  createdAt?: Timestamp | null;
  updatedAt?: Timestamp | null;
}

export interface Ocr {
  [key: string]: unknown;
  ndl?: OcrResult | null;
  minna?: OcrResult | null;
}

export interface User {
  [key: string]: unknown;
  uid: string;
  displayName: string;
}

export interface AppError { kind: string; message: string }

export type OcrResult = string | OcrRecord;
export interface OcrRecord {
  [key: string]: unknown;
  text: string;
  createdAt?: Timestamp | null;
  textBlocks?: JsonValue[] | null;
}
