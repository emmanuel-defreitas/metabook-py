import os

import pytest

from tf_ref import node_to_ref, resolve_ref


@pytest.mark.skipif(
    not os.environ.get("TF_REF_SMOKE_APP"),
    reason="set TF_REF_SMOKE_APP to a locally installed Text-Fabric app",
)
def test_local_text_fabric_corpus_round_trip():
    tf_app = pytest.importorskip("tf.app")
    app = tf_app.use(os.environ["TF_REF_SMOKE_APP"], checkout="local", silent="deep")
    ref = os.environ.get("TF_REF_SMOKE_REF", "Genesis:1:1")
    node = resolve_ref(ref, app)
    assert node_to_ref(node, app).endswith(f"/{ref}")
