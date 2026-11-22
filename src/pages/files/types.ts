export type FileScope = "folder" | "workspace";

export interface MediaStreamInfo {
  type?: string | null;
  codec?: string | null;
  width?: number | null;
  height?: number | null;
  channels?: number | null;
  sample_rate?: string | null;
}

export interface MediaInfo {
  format?: string | null;
  duration_secs?: number | null;
  bit_rate?: number | null;
  streams: MediaStreamInfo[];
}

export interface Crumb {
  label: string;
  path: string;
}
