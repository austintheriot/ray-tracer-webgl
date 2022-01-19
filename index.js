import './style.scss';

const main = async () => {
  const { main: wasmMain, save_image } = await import('./pkg');
  document.querySelector('button').onclick = save_image;
  wasmMain();
}

main();
