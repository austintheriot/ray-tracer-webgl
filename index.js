import './style.scss';

const main = async () => {
  const canvas = document.querySelector('canvas');
  const saveImageButton = document.querySelector('button');

  saveImageButton.onclick = () => {
    const a = document.createElement('a');
    a.href = canvas.toDataURL("image/png").replace("image/png", "image/octet-stream");
    a.download = 'canvas.png';
    a.click();
  };

  const { main: wasmMain } = await import('./pkg');

  wasmMain();
}

main();
