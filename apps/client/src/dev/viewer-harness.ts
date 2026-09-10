import OpenSeadragon from "openseadragon";
declare global {
  interface Window {
    honkokuViewer: () => OpenSeadragon.Viewer | undefined;
  }
}
window.honkokuViewer = () => {
  const element = document.querySelector<HTMLElement>(".osd");
  return element ? OpenSeadragon.getViewer(element) : undefined;
};
