export type ChapterRegenerationWorkflowResult = {
  ok: boolean;
};

export async function startChapterRegenerationWorkflow(): Promise<ChapterRegenerationWorkflowResult> {
  return { ok: true };
}
