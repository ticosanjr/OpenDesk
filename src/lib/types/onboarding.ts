export interface OnboardingStep {
  id: string;
  label: string;
  detail: string;
  status: "done" | "todo";
  href: string;
  required: boolean;
}

export interface OnboardingStatus {
  steps: OnboardingStep[];
  required_done: number;
  required_total: number;
  optional_done: number;
  optional_total: number;
  complete: boolean;
}
