# Forest understory

Warm forests: arching fern, broadleaf shrub, bramble, wood sorrel, forest lily,
woodland sedge, bluebell, bearberry and clubmoss.

Boreal forests: shield fern, bilberry, heather, dwarf willow, creeping juniper,
cotton sedge, bunchberry, bearberry and clubmoss.

Bearberry and clubmoss share morphology between climates, with snow added to
their boreal appearances: 16 distinct species, nine per forest. These are stylized
plant forms, not botanical simulations. Plants are decorative, not collectible.
Winter variants carry raised, tapered snow caps along branch forks, exposed
stems, fern arches and selected leaf surfaces. Coverage varies between variants;
sheltered stems remain exposed. Snow is part of the shared mesh, with no extra
materials, entities or draw calls.

Each appearance has six world-seeded procedural morphologies. Frond count,
length, curvature, leaf proportions, branching direction and coloration vary;
individual placements also vary in rotation, width and height. Returning to a
location restores the same plants rather than rerolling the scenery.
Eighteen-meter colonies favor one species on 60% of placements; the remaining
40% mixes other species. This adds recognizable patches without increasing density.

Placement uses one jittered candidate per 9 square meters, with 12–54% acceptance
in full forest depending on the local patch mask (roughly 1–6 plants per 100
square meters before trunk exclusion). Forest edges thin out. The snowy biome
transition is left clear, and plains, tundra, trunks and the castle are excluded.

All plants share one opaque, rough, double-sided vertex-colored material and a
bank of 108 meshes. No per-plant textures, shadow passes or update systems.
Twelve-meter cells stream four at a time, nearest first, with entity eviction
outside the configured shared render range. Morphology assets are reused, not
rebuilt during movement. The camera's finite far plane clips plants consistently
with the rest of the vegetation.
