"""Shared anatomical finger poses for the viewmodel and exterior avatar."""


def finger_angles(clenched, side="r"):
    angles = {}
    for i, finger in enumerate(("index", "middle", "ring", "pinky")):
        values = ((1.12+i*.025, 1.38, .90) if clenched else
                  (.10+i*.025, .24+i*.045, .16+i*.025))
        for segment, angle in zip(("01", "02", "03"), values):
            angles[f"{finger}_{segment}_{side}"] = angle
    for segment, angle in zip(("01", "02", "03"),
                              (.45, .70, .45) if clenched else (0, 0, 0)):
        angles[f"thumb_{segment}_{side}"] = angle
    return angles
