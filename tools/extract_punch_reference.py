"""Extract a right straight punch from CMU 14_01.c3d (120 Hz).

Usage: tools/.venv/bin/python tools/extract_punch_reference.py path/to/14_01.c3d
Requires numpy and ezc3d in the authoring environment; neither is used by the game.
The CMU FAQ permits copying, modifying and redistributing this motion data.
"""
import json
import sys
from pathlib import Path
import numpy as np
import ezc3d

START, IMPACT, END = 1339, 1357, 1381  # zero-based samples within this C3D
ROOT = Path(__file__).resolve().parents[1]

def unit(v):
    return v / np.linalg.norm(v, axis=-1, keepdims=True)

def extract(path):
    capture = ezc3d.c3d(str(path))
    assert capture['header']['points']['frame_rate'] == 120
    names = [n.split(':')[-1] for n in capture['parameters']['POINT']['LABELS']['value']]
    points = capture['data']['points'][:3].transpose(2, 1, 0)
    def point(name):
        return points[:, names.index(name)]
    # Vicon model output joint centers, rather than surface-marker positions:
    # clavicle endpoint = shoulder, humerus endpoint = elbow, wrist endpoint.
    shoulder, elbow, wrist = (point(n) for n in ('RCL0', 'RHU0', 'RWR0'))
    left = unit((point('LCL0')-shoulder)*np.array([1, 1, 0]))
    up = np.tile([0, 0, 1], (len(points), 1))
    back = np.cross(up, left)
    # Freeze the reference frame at the start. A frame that follows the
    # shoulders cancels their rotation and collapses the elbow's outward arc.
    # Preserve the recorded strike heading, including its lateral component.
    basis = np.stack([left[START], back[START], up[START]])
    local = lambda v: v @ basis.T
    forward = unit(point('RHN0')-wrist)
    across = point('RWRB')-point('RWRA')
    across = unit(across-forward*np.sum(across*forward, axis=1, keepdims=True))
    vectors = {
        'upper': elbow-shoulder,
        'lower': wrist-elbow,
        'lower_normal': np.cross(point('RWRB')-point('RWRA'), unit(wrist-elbow)),
        'hand_forward': forward,
        'hand_normal': np.cross(across, forward),
        'clavicle': shoulder-point('TRX0'),
    }
    tracks = {}
    for name, values in vectors.items():
        values = unit(local(unit(values)))
        # Symmetric five-sample binomial filter suppresses marker jitter without
        # delaying the motion or replacing its measured elbow path.
        tracks[name] = unit(sum(weight*values[START+offset:END+offset+1]
            for offset, weight in zip(range(-2, 3), [1, 4, 6, 4, 1])) / 16)
        assert np.isfinite(tracks[name]).all()
    doc = {
        'source': 'Carnegie Mellon University Graphics Lab Motion Capture Database',
        'source_url': 'https://mocap.cs.cmu.edu/subjects/14/14_01.c3d',
        'description_url': 'https://mocap.cs.cmu.edu/search.php?subjectnumber=14',
        'permission_url': 'https://mocap.cs.cmu.edu/faqs.php',
        'capture': '14_01 (boxing)', 'sample_indices_zero_based': [START, END],
        'impact_sample_zero_based': IMPACT, 'sample_rate': 120,
        'duration': (END-START)/120, 'impact_time': (IMPACT-START)/120,
        'processing': 'Fixed start-frame directions, original lateral heading and shoulder rotation, symmetric five-sample binomial filter; original timing retained.',
        'samples': [{key: [round(float(v), 8) for v in track[i]]
                     for key, track in tracks.items()} for i in range(END-START+1)],
    }
    output = ROOT/'assets/avatars/source/punch-reference.json'
    output.write_text(json.dumps(doc, indent=2)+'\n')
    print(output, len(doc['samples']), 'recorded samples')

if __name__ == '__main__':
    extract(Path(sys.argv[1]))
