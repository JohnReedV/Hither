#!/usr/bin/env python3
"""Regression for reverse-Z self-occlusion from independent shader rounding.

Run with python3 tools/check_sdf_depth.py. Uses explicit float32 rounding so it
also runs without a GPU; runtime shader/coverage checks remain necessary.
"""
from pathlib import Path
import struct
import unittest


def f32(x):
    return struct.unpack('f', struct.pack('f', x))[0]


def depth(distance, cosine, far, factored=False):
    near = f32(0.05)
    z = f32(distance * cosine)
    denominator = f32(far - near)
    if factored:
        value = f32(f32(near / denominator) * f32(f32(far / z) - 1.0))
    else:
        value = f32(f32(f32(f32(near * far) / z) - near) / denominator)
    return max(0.0, min(1.0, value))


class DepthPrecision(unittest.TestCase):
    def test_prepass_never_rejects_color_under_reassociation(self):
        shader = (Path(__file__).resolve().parents[1] / 'assets/shaders/sdf_scene.wgsl').read_text()
        self.assertIn('visible_travel * (64.0 * 1.1920928955078125e-7)', shader)
        self.assertIn('conservative_travel * dot(ray_direction, uniforms.camera_forward.xyz)', shader)
        # Final depth must remain exact: this preserves radial clipping and
        # ordinary geometry ordering after conservative occlusion decisions.
        self.assertIn('out.depth = hit_depth;', shader)
        exposed_old_failure = 0
        for far in [16.0, 48.0, 306.0, 1024.0]:
            for fraction in [0.001, 0.01, 0.1, 0.5, 1.0]:
                travel = f32(max(0.1, far * fraction))
                margin = f32(travel * f32(64.0 * 2.0**-23))
                conservative = f32(travel + margin)
                for i in range(1001):
                    cosine = f32(0.2 + 0.8 * i / 1000)
                    exact = [depth(travel, cosine, far, form) for form in [False, True]]
                    exposed_old_failure += exact[0] != exact[1]
                    for form in [False, True]:
                        self.assertLessEqual(depth(conservative, cosine, far, form), min(exact))
                    if fraction == 1.0:
                        # Reserve is small enough to continue culling geometry
                        # beyond the sky sphere (reverse-Z larger is nearer).
                        self.assertGreaterEqual(depth(conservative, cosine, far), depth(f32(far * 1.01), cosine, far))
                        self.assertGreaterEqual(depth(f32(far * .99), cosine, far), max(exact))
        self.assertGreater(exposed_old_failure, 1000)
        print(f'Checked 20,020 samples; {exposed_old_failure} expose unequal uncorrected depths.')


if __name__ == '__main__':
    unittest.main()
