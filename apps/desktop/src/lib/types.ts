export interface AppInfo {
  version: string;
  workspaceDir: string;
  dataDir: string;
  dockerAvailable: boolean;
  dockerVersion: string | null;
}

export interface DockerStatus {
  available: boolean;
  version: string | null;
}

export interface StackInfo {
  language: string;
  framework: string | null;
  packageManager: string | null;
  confidence: number;
  evidence: string[];
}

export interface PortSpec {
  container: number;
  protocol: string;
  healthPath: string | null;
}

export interface EnvSpec {
  key: string;
  required: boolean;
  secret: boolean;
  description: string | null;
}

export interface RunPlan {
  schemaVersion: number;
  stack: StackInfo;
  workdir: string;
  install: string[][];
  start: string[][];
  ports: PortSpec[];
  env: EnvSpec[];
  warnings: string[];
  planHash: string;
}

export interface Project {
  id: string;
  name: string;
  owner: string | null;
  repo: string | null;
  sourceUrl: string;
  requestedRef: string | null;
  resolvedCommit: string | null;
  subdir: string;
  localPath: string;
  state: string;
  depsReady: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface RunRecord {
  id: string;
  projectId: string;
  mode: string;
  state: string;
  startedAt: string | null;
  endedAt: string | null;
  exitCode: number | null;
  errorCode: string | null;
  logPath: string | null;
}

export interface ProjectDetail {
  project: Project;
  plan: RunPlan | null;
  runs: RunRecord[];
}

export interface RunInfo {
  runId: string;
  projectId: string;
  state: string;
}

export interface LogLine {
  stream: string;
  line: string;
}

export interface RunLogEvent extends LogLine {
  projectId: string;
  runId: string;
}

export interface RunStatusEvent {
  projectId: string;
  runId: string;
  state: string;
  message: string | null;
  exitCode: number | null;
}

export interface InstallProgress {
  message: string;
}

export interface AppError {
  code: string;
  title: string;
  message: string;
  details: string | null;
  hint: string | null;
  retryable: boolean;
}

export type View = "library" | "catalog" | "settings";
export type ProjectFilter = "all" | "running" | "stopped" | "error";
