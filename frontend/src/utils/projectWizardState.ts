export interface ProjectWizardStateLike {
  wizard_status?: 'incomplete' | 'completed' | string | null;
  wizard_step?: number | null;
}

export const isProjectWizardCompleted = (project: ProjectWizardStateLike | null | undefined): boolean => {
  if (!project) {
    return false;
  }

  if (project.wizard_status === 'completed') {
    return true;
  }

  return Number(project.wizard_step ?? 0) >= 4;
};

export const isProjectWizardIncomplete = (project: ProjectWizardStateLike | null | undefined): boolean => {
  if (!project) {
    return false;
  }

  return !isProjectWizardCompleted(project);
};
