import './style.scss';
import Worker from "worker-loader!./worker.js";

const main = async () => {
  const canvas = document.querySelector('canvas');
  const ctx = canvas.getContext('2d');
  const saveImageButton = document.querySelector('button');
  const worker = new Worker();

  saveImageButton.onclick = () => {
    const a = document.createElement('a');
    a.href = canvas.toDataURL("image/png").replace("image/png", "image/octet-stream");
    a.download = 'canvas.png';
    a.click();
  };

  // receive image data from main thread
  worker.onmessage = (e) => {
    console.log("Message received in main thread: ", e.data);
    if (!('imageData' in e.data)) return;
    const { imageData, width, height } = e.data;

    canvas.width = width;
    canvas.height = height;

    ctx.putImageData(imageData, 0, 0);
  }

  // begin ray tracing
  worker.postMessage('start');
}

main();
