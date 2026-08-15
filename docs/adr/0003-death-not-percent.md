# 0003. Stop on death, not on a percent floor

Status: accepted

Firmware 0% while audio still flows is a finding, not a stop. A soak ends only after grace when the Device is gone and stays gone, or when playback is silent while still listed as connected, or when the soak is stopped. There is no floor picker and no safety cap; a mains-powered speaker plays until stopped. False death that comes back resumes the soak as a segment.
