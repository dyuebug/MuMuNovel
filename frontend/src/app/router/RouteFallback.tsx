import LoadingScreen from '../../components/LoadingScreen';

export default function RouteFallback() {
  return <LoadingScreen message="加载中..." minHeight="40vh" />;
}
