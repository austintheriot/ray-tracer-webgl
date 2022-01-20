import './style.scss';

(async () => {
  (await import('./pkg')).main();
})();
