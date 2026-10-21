export const getCurrentWindow = () => ({
  isFullscreen: async () => false,
  isMaximized: async () => true,
  setTitle: async () => {},
  close: async () => {},
  minimize: async () => {},
  startDragging: async () => {},
});
