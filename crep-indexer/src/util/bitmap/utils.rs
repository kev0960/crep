use roaring::RoaringBitmap;

pub fn intersect_bitmaps(bitmaps: &[&RoaringBitmap]) -> Option<RoaringBitmap> {
    let mut iter = bitmaps.iter();
    let first = (*iter.next()?).clone();

    let result = iter.fold(first, |mut total, bitmap| {
        total &= *bitmap;
        total
    });

    Some(result)
}

pub fn intersect_bitmap_vec(
    mut bitmaps: Vec<RoaringBitmap>,
) -> Option<RoaringBitmap> {
    if bitmaps.is_empty() {
        return None;
    }

    let mut acc = bitmaps.pop().unwrap();

    for bm in bitmaps {
        acc &= bm;
        if acc.is_empty() {
            break;
        }
    }

    Some(acc)
}

pub fn union_bitmaps(bitmaps: &[&RoaringBitmap]) -> Option<RoaringBitmap> {
    let mut iter = bitmaps.iter();
    let first = (*iter.next()?).clone();

    let result = iter.fold(first, |mut total, bitmap| {
        total |= *bitmap;
        total
    });

    Some(result)
}
