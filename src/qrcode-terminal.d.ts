declare module 'qrcode-terminal' {
  interface Options {
    small?: boolean;
  }
  function generate(text: string, opts?: Options, cb?: (code: string) => void): void;
  export default { generate };
}
