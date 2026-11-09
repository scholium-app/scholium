"""Review images are opt-in; pixel assertions never use this switch."""
import os


def evidence_enabled():
    return os.environ.get('SCHOLIUM_CHECK_EVIDENCE') == '1'
