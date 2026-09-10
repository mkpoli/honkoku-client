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

export interface SaveOptions {
  status?: "initiated" | "completed";
  share?: boolean;
  requestReview?: boolean;
  comment?: string;
  isApproval?: boolean;
}
export interface SavedPage {
  page: Page;
  timelineEventId: string;
}
export interface PageLockState {
  pageId: string;
  status: PageStatus;
  tempEditedBy: string | null;
  isMine: boolean;
  syncMode: boolean;
  updateTime: string;
}

export interface SavedPage {
  /** Character count from the save's timeline event, when supplied by the backend. */
  count?: number;
}

export type SignInProvider = "google.com" | "twitter.com";

/** Native saves include the count read from the committed timeline event. */
export interface SavedPageWithCount extends SavedPage {
  count: number;
}

export interface EntrySummary {
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
}
export interface StatusCounts {
  completed: number;
  initiated: number;
  editing: number;
}
export interface CollectionProgress extends StatusCounts {
  collectionId: string;
  entries: number;
  size: number;
  fetchedAt: Timestamp;
}
export interface EntryProgress extends StatusCounts {
  entryId: string;
  size: number;
  fetchedAt: Timestamp;
}

export interface LocalOcrLine {
  reading_order: number;
  x: number;
  y: number;
  width: number;
  height: number;
  confidence: number;
  koji: string;
  plain: string;
  raw: string;
}
export interface LocalOcrPage {
  width: number;
  height: number;
  processed_width: number;
  processed_height: number;
  model: string;
  created_at: string;
  lines: LocalOcrLine[];
  timings: Record<string, number>;
  warnings: string[];
}
export interface OcrStatus {
  version: string | null;
  device: string;
  environment_ready: boolean;
  models_ready: boolean;
  model_version: string;
  cuda_available: boolean;
}
export interface OcrProgress {
  id: number;
  stage: string;
  done: number;
  total: number;
  message: string;
}

export type CredentialStore = "os" | "file";
export interface SessionInfo {
  credential_store?: CredentialStore;
}

export interface RecentWork {
  entryId: string;
  index: number;
  projectId: string;
  openedAt: Timestamp;
  savedAt: Timestamp | null;
  statusAfter: PageStatus;
  entryLabel: Label | null;
  projectTitle: string | null;
  thumbnail: string | null;
  nextUnfinishedIndex: number | null;
}

export interface PageLockState {
  /** Fresh server snapshot returned with the lock check. */
  page?: Page;
}

export interface OcrDiagnostics {
  status: OcrStatus | null;
  environment_ready: boolean;
  models_present: boolean;
  models_directory_exists: boolean;
  models_bytes: number;
  last_error: string | null;
  log_path: string;
}

export type SearchMode = "Strict" | "Folded";
export interface SearchQuery {
  text: string;
  mode: SearchMode;
  project: string | null;
  entry: string | null;
  limit: number;
  cursor: string | null;
}
export interface SearchOccurrence {
  original_start: number;
  original_end: number;
  before: string;
  matched: string;
  after: string;
  column: number;
}
export interface SearchHit {
  page_id: string;
  project_id: string;
  entry_id: string;
  index: number;
  entry_label: string;
  project_title: string;
  occurrences: SearchOccurrence[];
}
export interface SearchResults {
  total: number;
  hits: SearchHit[];
  facets: [string, number][];
  next_cursor: string | null;
}
export interface SearchStatus {
  present: boolean;
  commit: string | null;
  page_count: number;
  last_build: number | null;
  size: number;
  git_available?: boolean;
  configured?: boolean;
}
export interface SearchProgress {
  done: number;
  total: number;
  indexed: number;
}

export interface GlyphOccurrence {
  column: number;
  plain: string;
  offset: number;
  before: string;
  matched: string;
  after: string;
}
export interface GlyphAttestation {
  pageId: string;
  entryId: string;
  projectId: string;
  index: number;
  entryLabel: string;
  projectTitle: string;
  canvas: Canvas | null;
  text: string;
  ocr: Page["ocr"];
  occurrences: GlyphOccurrence[];
  error: string | null;
}
export interface GlyphAttestations {
  total: number;
  facets: [string, number][];
  pages: GlyphAttestation[];
  firestoreReads: number;
}
export interface ClipInput {
  entryId: string;
  index: number;
  reading: string;
  tags: string[];
  comment: string;
  isPrivate: boolean;
  xywh: [number, number, number, number];
}
export interface Clip extends ClipInput {
  id: string;
  uid: string;
  uri: string;
  transcriptionId: string;
  projectId: string;
  createdAt: string;
}
