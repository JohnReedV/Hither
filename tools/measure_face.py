"""Measure face landmarks locally; never sends reference portraits to a server."""
import json
from pathlib import Path
import sys
import mediapipe as mp

root = Path(__file__).resolve().parents[1]
(root / "tools/build").mkdir(parents=True, exist_ok=True)
options = mp.tasks.vision.FaceLandmarkerOptions(
    base_options=mp.tasks.BaseOptions(model_asset_path=str(root / "tools/vendor/face_landmarker.task")),
    num_faces=1,
)
with mp.tasks.vision.FaceLandmarker.create_from_options(options) as detector:
    faces = []
    for path in sys.argv[1:]:
        image = mp.Image.create_from_file(path)
        result = detector.detect(image)
        if not result.face_landmarks:
            raise ValueError("No face detected: " + path)
        faces.append({"path": path, "size": [image.width, image.height],
                      "points": [[p.x, p.y, p.z] for p in result.face_landmarks[0]]})
    (root / "tools/build/face-measurements.json").write_text(json.dumps(faces))
    print("Measured", len(faces), "faces locally")
