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

pub fn union_bitmaps(bitmaps: &[&RoaringBitmap]) -> Option<RoaringBitmap> {
    let mut iter = bitmaps.iter();
    let first = (*iter.next()?).clone();

    let result = iter.fold(first, |mut total, bitmap| {
        total |= *bitmap;
        total
    });

    Some(result)
}
