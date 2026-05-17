import { useLocation, useNavigate } from 'react-router-dom';
import { BackgroundTaskCenterView } from '../features/background-tasks/ui/BackgroundTaskCenterView';
import { useBackgroundTaskCenterController } from '../features/background-tasks/hooks/useBackgroundTaskCenterController';

export default function BackgroundTaskCenter() {
  const location = useLocation();
  const navigate = useNavigate();

  const controller = useBackgroundTaskCenterController({
    pathname: location.pathname,
    navigate,
    isMobile: false,
  });

  if (controller.hiddenByRoute || controller.visibleTaskCount === 0) {
    return null;
  }

  return <BackgroundTaskCenterView controller={controller} />;
}
