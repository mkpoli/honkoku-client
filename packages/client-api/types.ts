export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };
export type Timestamp = string;
export type PageStatus =
  | "default"
  | "initiated"
  | "editing"
  | "completed"
  | "frozen"
  | (string & {});
export type Label = string | Record<string, string[]>;
export type Keywords = string | string[];

export interface Project {
  [key: string]: unknown;
  id: string;
  title: string;
  isPrivate?: boolean | null;
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
  attribution?: Label | null;
  license?: string | null;
  initiated?: number | null;
  progress?: number | null;
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
  level?: number | null;
  exp?: number | null;
  charCount?: number | null;
  likeCount?: number | null;
  stoneCount?: number | null;
  photoURL?: string | null;
  profile?: string | null;
  createdAt?: Timestamp | null;
  updatedAt?: Timestamp | null;
}

export interface AppError {
  kind: string;
  message: string;
}

export type OcrResult = string | OcrRecord;
export interface OcrRecord {
  [key: string]: unknown;
  text: string;
  createdAt?: Timestamp | null;
  textBlocks?: JsonValue[] | null;
}

export interface SessionInfo {
  uid: string;
  display_name: string | null;
  providers: string[];
}
export interface TimelineFilter {
  project_id?: string;
  joined?: boolean;
  before?: Timestamp;
  before_id?: string;
}
export type RankingSort = "exp" | "charCount" | "likeCount";
export interface TimelineEvent {
  [key: string]: unknown;
  id: string;
  uid: string;
  projectId: string;
  entryId: string;
  transcriptionId: string;
  index: number;
  eventType: string;
  count: number;
  isReview: boolean;
  share?: boolean | null;
  requestReview?: boolean | null;
  isApproval?: boolean | null;
  createdAt: Timestamp;
  data: JsonValue;
}
export interface TimelineItem {
  event: TimelineEvent;
  actor: User | null;
  entryLabel: Label | null;
  projectTitle: string | null;
  excerpt: string;
}
export interface Announcement {
  [key: string]: unknown;
  id: string;
  title: string;
  description: string;
  display: boolean;
  createdAt: Timestamp;
}
export interface Notification {
  [key: string]: unknown;
  id: string;
  uid: string;
  type: string;
  data: JsonValue;
  state: string;
  createdAt: Timestamp;
}
export type NonFiniteNumber = "NaN" | "Infinity" | "-Infinity";
export type ProgressCount = number | NonFiniteNumber;
export interface ProjectProgress {
  [key: string]: unknown;
  totalEntryCount: number;
  completedEntryCount: number;
  totalImageCount: number;
  completedImageCount: ProgressCount;
  charCount: number;
  participants: Record<string, boolean>;
}
export interface DailyProgress {
  [key: string]: unknown;
  id: string;
  timestamp: Timestamp;
  totalEntryCount: number;
  completedEntryCount: number;
  totalImageCount: number;
  completedImageCount: number;
  initiatedImageCount: number;
  defaultImageCount: number;
  charCount: number;
  userCount: number;
  projects: Record<string, ProjectProgress>;
}
export interface PageNote {
  id?: string;
  type?: string;
  content: string;
  markdown?: string;
  createdBy?: string;
  createdAt?: Timestamp;
  updatedAt?: Timestamp;
}
