// Delirium (github.com/PVCPC/delirium-physics, a port of GTA IV's Euphoria) as the brain of ordinary ragdolls: the game's
// ragdoll stays a ragdoll, simulated here with everything else, and a character's brain (the gmsv_delirium module)
// reads its bodies and works its joints. Each muscle is a spring on the ragdoll's own joint, pulling the attached body
// towards a rotation - solved inside Box3D's step, every substep, with the contacts and limits - and what its
// behaviours add beyond the muscles (holding up against gravity, the balancer's help, a shot's push) comes as forces.
//
// Plain C functions, found by name by the module (GetProcAddress / dlsym). Objects are passed as the game's
// IPhysicsObject pointers; one that no longer exists is refused, never touched.

#include "physics_constraint.h"
#include "physics_environment.h"
#include "physics_object.h"
#include "conversion.h"
#include "box3d/box3d.h"
#include "box3d/math_functions.h"
#include <cmath>
#include <unordered_set>

#if defined( _WIN32 )
#define DELIRIUM_API extern "C" __declspec( dllexport )
#else
#define DELIRIUM_API extern "C" __attribute__( ( visibility( "default" ) ) )
#endif

// Every physics object alive (kept by CPhysicsObject itself).
std::unordered_set<const IPhysicsObject *> &DeliriumLiveObjects()
{
	static std::unordered_set<const IPhysicsObject *> s_Objects;
	return s_Objects;
}

static CPhysicsObject *Live( void *pObject )
{
	if ( !pObject )
		return nullptr;
	const IPhysicsObject *p = static_cast<const IPhysicsObject *>( pObject );
	if ( DeliriumLiveObjects().find( p ) == DeliriumLiveObjects().end() )
		return nullptr;
	return static_cast<CPhysicsObject *>( const_cast<IPhysicsObject *>( p ) );
}

// 1: the functions below, as they are here.
DELIRIUM_API int VBox3D_Delirium_Version()
{
	return 1;
}

// An object's state: position (units), rotation (quaternion x y z w, the game's axes), velocity (units/s), angular
// velocity (radians/s, world). 13 floats. 0 if there's no such object.
DELIRIUM_API int VBox3D_Delirium_State( void *pObject, float *pOut )
{
	CPhysicsObject *pObj = Live( pObject );
	if ( !pObj || !pOut )
		return 0;
	Vector position;
	QAngle angles;
	pObj->GetPosition( &position, &angles );
	Vector velocity;
	AngularImpulse local;
	pObj->GetVelocity( &velocity, &local );
	const b3Quat q = ToBox3D( angles );
	const b3Vec3 w = b3RotateVector( q, { local.x * DEGREES_TO_RADIANS, local.y * DEGREES_TO_RADIANS, local.z * DEGREES_TO_RADIANS } );
	const float out[13] = { position.x, position.y, position.z, q.v.x, q.v.y, q.v.z, q.s, velocity.x, velocity.y, velocity.z, w.x, w.y, w.z };
	for ( int i = 0; i < 13; ++i )
		pOut[i] = out[i];
	return 1;
}

// The object's mass (kg), and its environment's gravity (units/s/s, 3 floats). 0 if there's no such object.
DELIRIUM_API int VBox3D_Delirium_Mass( void *pObject, float *pMass, float *pGravity )
{
	CPhysicsObject *pObj = Live( pObject );
	if ( !pObj )
		return 0;
	if ( pMass )
		*pMass = pObj->GetMass();
	if ( pGravity )
	{
		Vector g;
		pObj->GetEnvironment()->GetGravity( &g );
		pGravity[0] = g.x;
		pGravity[1] = g.y;
		pGravity[2] = g.z;
	}
	return 1;
}

// A muscle on the joint between two objects (a ragdoll's own constraint between them, either way round): it pulls
// 'child' towards the rotation pRotation (x y z w) in 'parent''s frame (the game's frames of the two objects), as a
// spring of 'hertz' (cycles a second; 0 or less: off) and 'damping' (the ratio: 1 is critical). 1 if the joint takes
// it; 0 if there's no such joint (or it can't: not a ball-and-socket or a hinge).
DELIRIUM_API int VBox3D_Delirium_Muscle( void *pParent, void *pChild, const float *pRotation, float hertz, float damping )
{
	CPhysicsObject *pParentObj = Live( pParent );
	CPhysicsObject *pChildObj = Live( pChild );
	if ( !pParentObj || !pChildObj || !pRotation )
		return 0;
	b3Quat q = { { pRotation[0], pRotation[1], pRotation[2] }, pRotation[3] };
	const float len = std::sqrt( q.v.x * q.v.x + q.v.y * q.v.y + q.v.z * q.v.z + q.s * q.s );
	if ( !( len > 1e-6f ) || !std::isfinite( len ) || !std::isfinite( hertz ) || !std::isfinite( damping ) )
		return 0;
	q = b3NormalizeQuat( q );
	for ( CPhysicsConstraint *pConstraint : pChildObj->GetConstraints() )
	{
		CPhysicsObject *pRef = pConstraint->GetReference();
		CPhysicsObject *pAtt = pConstraint->GetAttached();
		if ( pRef == pParentObj && pAtt == pChildObj )
			return pConstraint->SetMuscle( q, hertz, damping ) ? 1 : 0;
		if ( pRef == pChildObj && pAtt == pParentObj )
			return pConstraint->SetMuscle( b3Conjugate( q ), hertz, damping ) ? 1 : 0;
	}
	return 0;
}

// Pushes an object through the next step: a force at its centre of mass (newtons) and a torque (newton metres), world.
DELIRIUM_API int VBox3D_Delirium_Push( void *pObject, const float *pForce, const float *pTorque )
{
	CPhysicsObject *pObj = Live( pObject );
	if ( !pObj || pObj->IsStatic() || !pObj->IsMotionEnabled() )
		return 0;
	const b3BodyId body = pObj->GetBody();
	if ( !b3Body_IsValid( body ) )
		return 0;
	if ( pForce && std::isfinite( pForce[0] ) && std::isfinite( pForce[1] ) && std::isfinite( pForce[2] ) )
		b3Body_ApplyForceToCenter( body, { pForce[0] * IVP_TO_SOURCE, pForce[1] * IVP_TO_SOURCE, pForce[2] * IVP_TO_SOURCE }, true );
	if ( pTorque && std::isfinite( pTorque[0] ) && std::isfinite( pTorque[1] ) && std::isfinite( pTorque[2] ) )
	{
		constexpr float k = IVP_TO_SOURCE * IVP_TO_SOURCE;
		b3Body_ApplyTorque( body, { pTorque[0] * k, pTorque[1] * k, pTorque[2] * k }, true );
	}
	return 1;
}
