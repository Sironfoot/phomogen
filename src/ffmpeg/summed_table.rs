// see https://en.wikipedia.org/wiki/Summed-area_table

pub struct SummedAreaTable {
    sums: Vec<[u32; 3]>,
    stride: u32,
    width: u32,
    height: u32,
}

impl SummedAreaTable {
    pub fn new(width: u32, height: u32) -> Self {
        let stride = width + 1;
        let sums = vec![[0u32; 3]; ((width + 1) * (height + 1)) as usize];

        Self {
            sums,
            stride,
            width,
            height,
        }
    }

    pub fn init(&mut self, pixels: &[u8]) {
        for y in 0..self.height {
            let mut row_red: u32 = 0;
            let mut row_green: u32 = 0;
            let mut row_blue: u32 = 0;

            let src_row = y * self.width * 3;
            let dst_row = (y + 1) * self.stride;
            let prev_row = y * self.stride;

            for x in 0..self.width {
                let src = (src_row + x * 3) as usize;

                row_red += pixels[src] as u32;
                row_green += pixels[src + 1] as u32;
                row_blue += pixels[src + 2] as u32;

                let dst = (dst_row + x + 1) as usize;
                let above = (prev_row + x + 1) as usize;

                self.sums[dst][0] = self.sums[above][0] + row_red;
                self.sums[dst][1] = self.sums[above][1] + row_green;
                self.sums[dst][2] = self.sums[above][2] + row_blue;
            }
        }
    }

    #[inline]
    fn sum_rect(&self, x1: u32, y1: u32, x2: u32, y2: u32) -> [u32; 3] {
        let a = (y1 * self.stride + x1) as usize;
        let b = (y1 * self.stride + x2) as usize;
        let c = (y2 * self.stride + x1) as usize;
        let d = (y2 * self.stride + x2) as usize;

        [
            self.sums[d][0] + self.sums[a][0] - self.sums[b][0] - self.sums[c][0],
            self.sums[d][1] + self.sums[a][1] - self.sums[b][1] - self.sums[c][1],
            self.sums[d][2] + self.sums[a][2] - self.sums[b][2] - self.sums[c][2],
        ]
    }

    #[inline]
    pub fn average_rect(&self, x1: u32, y1: u32, x2: u32, y2: u32) -> [u8; 3] {
        let sum = self.sum_rect(x1, y1, x2, y2);
        let count = ((x2 - x1) * (y2 - y1)) as u32;

        [
            ((sum[0] + count / 2) / count) as u8,
            ((sum[1] + count / 2) / count) as u8,
            ((sum[2] + count / 2) / count) as u8,
        ]
    }
}