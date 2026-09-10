# Recorded punch reference

The bundled right-arm punch is retargeted from **Carnegie Mellon University
Graphics Lab Motion Capture Database, subject 14, trial 01 (boxing)**.

- Motion listing: https://mocap.cs.cmu.edu/search.php?subjectnumber=14
- Original recording: https://mocap.cs.cmu.edu/subjects/14/14_01.c3d
- Usage permission: https://mocap.cs.cmu.edu/faqs.php (the data may be copied,
  modified, or redistributed without permission).
- Selected zero-based samples within the C3D: **1339–1381**, at **120 Hz**.
  Maximum extension is sample **1357**. The recorded strike/recoil takes 0.35 s.

`punch-reference.json` contains the derived unit direction tracks. The extractor
uses the Vicon model outputs RCL0 (shoulder), RHU0 (elbow), RWR0 (wrist), and RHN0
(hand), plus the radial wrist markers for forearm/hand orientation. It converts
these to a fixed coordinate frame from the start of the punch, preserving the
recorded lateral heading and shoulder rotation, and applies a symmetric five-sample binomial filter to suppress marker jitter.

Reproduce with `tools/.venv/bin/python tools/extract_punch_reference.py
path/to/14_01.c3d`. The extractor requires `numpy` and `ezc3d`; the runtime does not.
The resulting JSON is the reproducible input to `tools/avatar_animation.py`.

The retargeter uses the recorded elbow bend plane, adapting segment lengths,
reach and timing to the bundled skeleton. The compact load flows into the
strike without a guard pause. Recoil samples the outgoing elbow path in reverse
to return directly to the starting guard. Chest rotation drives the rear shoulder forward;
the fist converges from the rear-hand guard toward the target at the face
centerline while the elbow stays outside that path. As the arm rises, the elbow pole
blends toward an outward bend while retaining a downward component. Extension reaches 98.7% of
arm length, preserving a soft elbow. The metacarpals align with the forearm at
impact and the knuckles turn up. Finger closure uses the shared first-person
fist pose.

The exported channels are the striking clavicle subtree, three spine bones,
and neck counter-turn. The free arm retains its locomotion pose and is carried
by the chest turn; the lower body keeps its locomotion channels. These are
procedural adaptations of the boxing capture, not an unmodified MMA recording.
The exterior clip is 0.60 s, with full extension at 0.22 s and recoil through
0.37 s. First-person timing remains 0.44 s, with phase mapping between views.

Visual technique references: Boxing Canada, *Instruction Beginners Reference
Manual* (2024), pp. 50 and 52; England Boxing, *Coaching Handbook Part 1*, p. 75.
Those references informed selection and inspection; they are not the motion data.
