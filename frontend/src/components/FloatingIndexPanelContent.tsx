import { memo } from 'react';
import FloatingIndexPanelResults from './FloatingIndexPanelResults';
import FloatingIndexPanelSearchHeader from './FloatingIndexPanelSearchHeader';
import type { FloatingIndexPanelViewModel } from '../utils/floatingIndexPanelContracts';

type FloatingIndexPanelContentProps = {
  viewModel: FloatingIndexPanelViewModel;
};

function FloatingIndexPanelContent({ viewModel }: FloatingIndexPanelContentProps) {
  const { resultsModel, searchModel } = viewModel;

  return (
    <>
      <FloatingIndexPanelSearchHeader searchModel={searchModel} />
      <FloatingIndexPanelResults resultsModel={resultsModel} />
    </>
  );
}

export default memo(FloatingIndexPanelContent);