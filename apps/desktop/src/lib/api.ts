import { invoke } from "@tauri-apps/api/core";
import type {
  AppInfo,
  DockerStatus,
  LogLine,
  Project,
  ProjectDetail,
  RunInfo,
  RunRecord,
} from "./types";

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  dockerStatus: () => invoke<DockerStatus>("docker_status"),
  addProject: (url: string) => invoke<ProjectDetail>("add_project", { url }),
  listProjects: () => invoke<Project[]>("list_projects"),
  projectDetail: (projectId: string) =>
    invoke<ProjectDetail>("project_detail", { projectId }),
  removeProject: (projectId: string, deleteFiles: boolean) =>
    invoke<void>("remove_project", { projectId, deleteFiles }),
  startProject: (projectId: string) =>
    invoke<RunInfo>("start_project", { projectId }),
  stopProject: (projectId: string) =>
    invoke<void>("stop_project", { projectId }),
  listRuns: (projectId: string) =>
    invoke<RunRecord[]>("list_runs", { projectId }),
  readLog: (projectId: string, runId: string) =>
    invoke<LogLine[]>("read_log", { projectId, runId }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  openProjectFolder: (projectId: string) =>
    invoke<void>("open_project_folder", { projectId }),
  resetDependencies: (projectId: string) =>
    invoke<void>("reset_dependencies", { projectId }),
};
