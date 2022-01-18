const main = async () => {
  const [{ ray_trace, get_canvas_data }, { memory }] = await Promise.all([
    import('./pkg'),
    import('./pkg/index_bg')
  ]);

  ray_trace();

  // get pointer to canvas memory and load pixel data
  let canvasData = get_canvas_data();
  let canvasPixelData = new Uint8ClampedArray(memory.buffer, canvasData.pixels_ptr, canvasData.pixels_len);
  const imageData = new ImageData(canvasPixelData, canvasData.canvas_width, canvasData.canvas_height);
  postMessage({ imageData, width: canvasData.canvas_width, height: canvasData.canvas_height });
}

onmessage = (e) => {
  console.log("Message received in worker: ", e.data);
  if (e.data === 'start') {
    main();
  }
}