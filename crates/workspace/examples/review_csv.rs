//! Review the straight first-to-last segment of an input-only baseline CSV.
//! Read stdin; no hardware access or implied selection of a physical origin.
use haptics_workspace::{EvidenceCloud, MAX_POINTS, Policy, SegmentReview};
use std::io::{self, BufRead};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut lines = io::stdin().lock().lines();
    let header = lines.next().ok_or("missing CSV header")??;
    let columns: Vec<_> = header.split(',').collect();
    let indices: Vec<_> = ["model_x_m", "model_y_m", "model_z_m"]
        .into_iter()
        .map(|name| {
            columns
                .iter()
                .position(|s| *s == name)
                .ok_or("missing model position column")
        })
        .collect::<Result<_, _>>()?;
    let mut points = Vec::new();
    for line in lines {
        if points.len() == MAX_POINTS {
            return Err("too many input samples".into());
        }
        let line = line?;
        let values: Vec<_> = line.split(',').collect();
        let mut p = [0.; 3];
        for i in 0..3 {
            p[i] = values.get(indices[i]).ok_or("short CSV row")?.parse()?;
        }
        points.push(p);
    }
    let start = *points.first().ok_or("empty recording")?;
    let end = *points.last().unwrap();
    let cloud = EvidenceCloud::new(points, Policy::default())?;
    println!(
        "Observed points: {}; effective sample neighbourhood: 3 model mm",
        cloud.points().len()
    );
    println!("First recorded pose: {start:?}; last recorded pose: {end:?}");
    match cloud.review_segment(start, end)? {
        SegmentReview::Supported => {
            println!("Straight segment lies within sample-supported neighbourhoods.")
        }
        SegmentReview::Gap { from, to } => println!(
            "First unsupported segment: {:.6}% .. {:.6}% of path.",
            100. * from,
            100. * to
        ),
    }
    println!("This is evidence proximity, not physical path clearance or motor authorization.");
    Ok(())
}
