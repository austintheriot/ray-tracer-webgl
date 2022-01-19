import './style.scss';

const main = async () => {
  const canvas = document.querySelector('canvas');
  const saveImageButton = document.querySelector('button');

  const { main: wasmMain, save_image } = await import('./pkg');

  saveImageButton.onclick = save_image;

  wasmMain();
}

main();
