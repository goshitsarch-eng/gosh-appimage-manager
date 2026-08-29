#pragma once

#include <QString>

namespace GoshAim {

class UpdateNotifier
{
public:
    virtual ~UpdateNotifier() = default;
    virtual void notifyUpdatesAvailable(int count) = 0;
};

class NullUpdateNotifier : public UpdateNotifier
{
public:
    void notifyUpdatesAvailable(int count) override;
};

class KdeUpdateNotifier : public UpdateNotifier
{
public:
    void notifyUpdatesAvailable(int count) override;
};

class RecordingUpdateNotifier : public UpdateNotifier
{
public:
    int calls = 0;
    int lastCount = 0;
    void notifyUpdatesAvailable(int count) override
    {
        if (count <= 0) {
            return;
        }
        ++calls;
        lastCount = count;
    }
};

} // namespace GoshAim
