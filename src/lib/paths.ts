export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

export const fileStem = (path: string) => fileName(path).replace(/\.pdf$/i, "");
