import { Navigate, useParams } from 'react-router-dom';

import { NovelAutopilotWorkbench } from '../features/novel-autopilot';

const NovelAutopilot = () => {
  const { projectId } = useParams<{ projectId: string }>();
  if (!projectId) {
    return <Navigate to="/projects" replace />;
  }
  return <NovelAutopilotWorkbench projectId={projectId} />;
};

export default NovelAutopilot;
