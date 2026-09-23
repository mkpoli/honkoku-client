export interface LineBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * OCR boxes hug the ink and routinely clip the first and last glyph of a
 * column. The drawn frame therefore sits outside a generously padded rectangle,
 * grown mostly along the line.
 */
export function lineFrameRect(line: LineBox): LineBox {
  const vertical = line.height >= line.width;
  const along = vertical ? line.height : line.width;
  const across = vertical ? line.width : line.height;
  const padAlong = Math.max(28, along * 0.18);
  const padAcross = Math.max(10, across * 0.14);
  const padX = vertical ? padAcross : padAlong;
  const padY = vertical ? padAlong : padAcross;
  return {
    x: line.x - padX,
    y: line.y - padY,
    width: line.width + padX * 2,
    height: line.height + padY * 2,
  };
}
